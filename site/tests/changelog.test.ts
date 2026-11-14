import { beforeAll, describe, expect, test } from "bun:test";
import { rmSync } from "node:fs";
import { buildSite } from "./build";

const withReleases = "dist-tests/changelog";
const empty = "dist-tests/changelog-empty";
const offline = "dist-tests/changelog-offline";
const unreleased = "dist-tests/changelog-unreleased";
const unreleasedOnly = "dist-tests/changelog-unreleased-only";

beforeAll(async () => {
  rmSync("dist-tests", { recursive: true, force: true });
  await Promise.all([
    buildSite(withReleases, {
      WREN_CHANGELOG_FIXTURE: "tests/fixtures/releases.json",
      WREN_STARS_FIXTURE: "1234",
    }),
    buildSite(empty, {
      WREN_CHANGELOG_FIXTURE: "tests/fixtures/no-releases.json",
      WREN_STARS_FIXTURE: "none",
    }),
    // A fixture that does not exist behaves like an offline build: warn, never fail.
    buildSite(offline, {
      WREN_CHANGELOG_FIXTURE: "tests/fixtures/missing.json",
      WREN_STARS_FIXTURE: "none",
    }),
    buildSite(unreleased, {
      WREN_CHANGELOG_FIXTURE: "tests/fixtures/releases.json",
      WREN_UNRELEASED_FIXTURE: "tests/fixtures/unreleased.json",
      WREN_STARS_FIXTURE: "none",
    }),
    buildSite(unreleasedOnly, {
      WREN_CHANGELOG_FIXTURE: "tests/fixtures/no-releases.json",
      WREN_UNRELEASED_FIXTURE: "tests/fixtures/unreleased.json",
      WREN_STARS_FIXTURE: "none",
    }),
  ]);
}, 240_000);

const read = (dir: string, file: string) => Bun.file(`${dir}/${file}`).text();

describe("changelog from a fixture", () => {
  test("lists the stable release and the pre-release with a badge", async () => {
    const html = await read(withReleases, "changelog/index.html");
    expect(html).toContain("Wren 0.1.0");
    expect(html).toContain("Wren 0.2.0 release candidate");
    expect(html).toContain("first fixture change");
    expect(html).toContain("Pre-release");
    // only the pre-release carries the badge
    expect(html.split("Pre-release").length - 1).toBe(1);
    expect(html).not.toContain("No releases yet");
  });

  test("spanish chrome is translated, release notes stay english", async () => {
    const html = await read(withReleases, "es/changelog/index.html");
    expect(html).toContain('<html lang="es"');
    expect(html).toContain("Versión preliminar");
    expect(html).toContain("Wren 0.1.0");
    expect(html).toContain("first fixture change");
    expect(html).not.toContain("Pre-release");
  });

  test("release notes are static html, not island props", async () => {
    const html = await read(withReleases, "changelog/index.html");
    expect(html).toContain("first fixture change");
    // the changelog is plain HTML: no island at all, so no props to leak into
    expect(html).not.toContain("<astro-island");
  });

  test("top nav links to the changelog", async () => {
    const html = await read(withReleases, "index.html");
    expect(html).toContain('href="/wren/changelog/"');
    const es = await read(withReleases, "es/index.html");
    expect(es).toContain('href="/wren/es/changelog/"');
  });
});

describe("unreleased section", () => {
  test("shows pending changes above the releases, marked as not released", async () => {
    const html = await read(unreleased, "changelog/index.html");
    expect(html).toContain('data-testid="changelog-unreleased"');
    expect(html).toContain("Unreleased");
    expect(html).toContain("Proposed version 0.3.0");
    expect(html).toContain("fixture pending feature");
    expect(html).toContain("fixture pending fix");
    expect(html).not.toContain("skipped chore");
    expect(html.indexOf("fixture pending feature")).toBeLessThan(
      html.indexOf("Wren 0.1.0"),
    );
  });

  test("spanish chrome is translated", async () => {
    const html = await read(unreleased, "es/changelog/index.html");
    expect(html).toContain("Sin publicar");
    expect(html).toContain("Versión propuesta 0.3.0");
    expect(html).toContain("fixture pending feature");
  });

  test("absent without unreleased data", async () => {
    const html = await read(withReleases, "changelog/index.html");
    expect(html).not.toContain('data-testid="changelog-unreleased"');
  });

  test("unreleased changes alone replace the empty state", async () => {
    const html = await read(unreleasedOnly, "changelog/index.html");
    expect(html).toContain('data-testid="changelog-unreleased"');
    expect(html).not.toContain('data-testid="changelog-empty"');
  });

  test("stays out of the atom feed", async () => {
    const xml = await read(unreleased, "changelog/atom.xml");
    expect(xml).toContain("Wren 0.1.0");
    expect(xml).not.toContain("0.3.0");
    expect(xml).not.toContain("fixture pending");
  });
});

describe("changelog without data", () => {
  test("zero releases renders the empty state", async () => {
    for (const file of ["changelog/index.html", "es/changelog/index.html"]) {
      const html = await read(empty, file);
      expect(html).toContain('data-testid="changelog-empty"');
    }
    expect(await read(empty, "changelog/index.html")).toContain(
      "No releases yet",
    );
    expect(await read(empty, "es/changelog/index.html")).toContain(
      "Todavía no hay",
    );
  });

  test("an unreadable source still builds the empty state", async () => {
    const html = await read(offline, "changelog/index.html");
    expect(html).toContain("No releases yet");
  });
});

describe("github stars", () => {
  test("renders the compact count in the landing page and the docs nav", async () => {
    for (const file of ["index.html", "es/index.html", "docs/index.html"]) {
      const html = await read(withReleases, file);
      expect(html).toMatch(/lucide-star[\s\S]{0,600}?1\.2k/);
    }
  });

  test("falls back to the plain link without a count", async () => {
    for (const file of [
      "index.html",
      "docs/index.html",
      "changelog/index.html",
    ]) {
      const html = await read(empty, file);
      expect(html).not.toContain("1.2k");
      expect(html).toContain("https://github.com/oscarbc96/wren");
    }
    expect(await read(empty, "index.html")).toContain("Source on GitHub");
  });
});

describe("theme switch", () => {
  test("offers light, dark and system, defaulting to system", async () => {
    for (const [file, labels] of [
      ["index.html", ["Light", "Dark", "System"]],
      ["docs/index.html", ["Light", "Dark", "System"]],
      ["changelog/index.html", ["Light", "Dark", "System"]],
      ["es/index.html", ["Claro", "Oscuro", "Sistema"]],
      ["es/docs/index.html", ["Claro", "Oscuro", "Sistema"]],
    ] as const) {
      const html = await read(withReleases, file);
      for (const label of labels) {
        expect(html).toContain(`aria-label="${label}"`);
      }
      // next-themes inline script runs before paint, with system as default
      expect(html).toContain("prefers-color-scheme: dark");
    }
  });
});
