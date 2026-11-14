import { readFile } from "node:fs/promises";
import { githubReleases } from "@fumadocs/changelog/github";
import type { GitHubChangelogEntry } from "@fumadocs/changelog/github";
import { fromCommits, parseReleasePr, type Unreleased } from "./unreleased";

/**
 * Build-time GitHub data (releases, stars). Nothing here may fail the build:
 * every failure logs a warning and falls back to "no data".
 */

export const owner = "oscarbc96";
export const repo = "wren";
export const repoUrl = `https://github.com/${owner}/${repo}`;
export const perPage = 30;

const token = process.env.GITHUB_TOKEN || undefined;

function warn(message: string, error?: unknown) {
  const detail = error instanceof Error ? `: ${error.message}` : "";
  console.warn(`[wren-site] ${message}${detail}`);
}

interface FixtureEntry extends Omit<GitHubChangelogEntry, "date"> {
  date: string;
}

async function fixtureReleases(path: string): Promise<GitHubChangelogEntry[]> {
  const raw = JSON.parse(await readFile(path, "utf8")) as FixtureEntry[];
  return raw.map((entry) => ({ ...entry, date: new Date(entry.date) }));
}

async function fetchReleases(): Promise<GitHubChangelogEntry[][]> {
  const fixture = process.env.WREN_CHANGELOG_FIXTURE;
  if (fixture) {
    const all = await fixtureReleases(fixture);
    const pages: GitHubChangelogEntry[][] = [];
    for (let i = 0; i < all.length; i += perPage) {
      pages.push(all.slice(i, i + perPage));
    }
    return pages;
  }

  const pages: GitHubChangelogEntry[][] = [];
  try {
    let page: number | undefined = 1;
    while (page !== undefined) {
      const result = await githubReleases({
        owner,
        repo,
        token,
        page,
        perPage,
      });
      if (result.entries.length > 0) pages.push(result.entries);
      page = result.entries.length > 0 ? result.nextPage : undefined;
    }
  } catch (error) {
    warn("could not fetch GitHub releases, keeping what was read", error);
  }
  return pages;
}

let releases: Promise<GitHubChangelogEntry[][]> | undefined;

/** Release pages, newest first. Empty when there are no releases or on any failure. */
export function getReleasePages(): Promise<GitHubChangelogEntry[][]> {
  releases ??= fetchReleases().catch((error) => {
    warn("could not load releases", error);
    return [];
  });
  return releases.then((pages) => {
    if (pages.length === 0)
      warn("no releases found, rendering the empty changelog");
    return pages;
  });
}

function apiHeaders(): Headers {
  const headers = new Headers({ accept: "application/vnd.github+json" });
  if (token) headers.set("authorization", `Bearer ${token}`);
  return headers;
}

async function api<T>(path: string): Promise<T | undefined> {
  const res = await fetch(
    `https://api.github.com/repos/${owner}/${repo}${path}`,
    {
      headers: apiHeaders(),
    },
  );
  if (!res.ok) {
    warn(`${path} unavailable (HTTP ${res.status})`);
    return undefined;
  }
  return (await res.json()) as T;
}

interface UnreleasedFixture {
  version?: string;
  subjects?: string[];
}

async function fetchUnreleased(): Promise<Unreleased | undefined> {
  const fixture = process.env.WREN_UNRELEASED_FIXTURE;
  if (fixture) {
    const raw = JSON.parse(
      await readFile(fixture, "utf8"),
    ) as UnreleasedFixture;
    const found = fromCommits(raw.subjects ?? [], repoUrl);
    return found ? { ...found, version: raw.version } : undefined;
  }
  // Other fixtures mean an offline, deterministic build.
  if (process.env.WREN_CHANGELOG_FIXTURE || process.env.WREN_STARS_FIXTURE)
    return undefined;

  // 1. The open release-please PR carries the proposed version and notes.
  const pending = await api<
    {
      title?: string;
      body?: string;
      html_url?: string;
      pull_request?: unknown;
    }[]
  >("/issues?state=open&labels=autorelease%3A%20pending&per_page=5");
  const pr = pending?.find((issue) => issue.pull_request);
  if (pr) {
    const parsed = parseReleasePr(pr);
    if (parsed) return parsed;
  }

  // 2. Otherwise the Conventional Commits on main since the latest release.
  const latest = (
    await api<{ tag_name?: string; draft?: boolean }[]>("/releases?per_page=5")
  )?.find((release) => !release.draft && release.tag_name);
  if (!latest?.tag_name) return undefined;
  const compare = await api<{ commits?: { commit: { message: string } }[] }>(
    `/compare/${encodeURIComponent(latest.tag_name)}...main`,
  );
  const subjects = (compare?.commits ?? []).map(
    (c) => c.commit.message.split("\n")[0] ?? "",
  );
  return fromCommits(subjects, repoUrl);
}

let unreleased: Promise<Unreleased | undefined> | undefined;

/** Pending changes for the top of the changelog; `undefined` when none or on any failure. */
export function getUnreleased(): Promise<Unreleased | undefined> {
  unreleased ??= fetchUnreleased().catch((error) => {
    warn("could not load the unreleased changes", error);
    return undefined;
  });
  return unreleased;
}

/** `1234` as `1.2k`, `999` as `999`. */
export function formatStars(count: number): string {
  if (count < 1000) return String(count);
  const value = count / 1000;
  const text =
    value < 10
      ? value.toFixed(1).replace(/\.0$/, "")
      : String(Math.round(value));
  return `${text}k`;
}

async function fetchStars(): Promise<number | undefined> {
  const fixture = process.env.WREN_STARS_FIXTURE;
  if (fixture !== undefined && fixture !== "") {
    const count = Number(fixture);
    return Number.isFinite(count) && count >= 0 ? count : undefined;
  }
  if (fixture === "") return undefined;
  try {
    const res = await fetch(`https://api.github.com/repos/${owner}/${repo}`, {
      headers: apiHeaders(),
    });
    if (!res.ok) {
      warn(`star count unavailable (HTTP ${res.status})`);
      return undefined;
    }
    const data = (await res.json()) as { stargazers_count?: unknown };
    return typeof data.stargazers_count === "number"
      ? data.stargazers_count
      : undefined;
  } catch (error) {
    warn("could not fetch the star count", error);
    return undefined;
  }
}

let stars: Promise<string | undefined> | undefined;

/** Compact star count (`1.2k`), `undefined` when unknown. */
export function getStars(): Promise<string | undefined> {
  stars ??= fetchStars().then((count) =>
    count === undefined ? undefined : formatStars(count),
  );
  return stars;
}
