import { readFile } from "node:fs/promises";
import { githubReleases } from "@fumadocs/changelog/github";
import type { GitHubChangelogEntry } from "@fumadocs/changelog/github";

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
    const headers = new Headers({ accept: "application/vnd.github+json" });
    if (token) headers.set("authorization", `Bearer ${token}`);
    const res = await fetch(`https://api.github.com/repos/${owner}/${repo}`, {
      headers,
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
