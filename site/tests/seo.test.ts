import { describe, expect, test } from "bun:test";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { gzipSync } from "node:zlib";
import config from "../src/i18n/config.json";

/**
 * Parses the built HTML in `dist/` (run `bun run build` first) and asserts the
 * SEO tags, the reciprocal hreflang sets and the JS/CSS byte budgets for every
 * page in every locale of `src/i18n/config.json`.
 */

const origin = "https://oscarbc96.github.io";
const base = "/wren";
const dist = "dist";
const codes = config.locales.map((l) => l.code);
const defaultLocale = config.defaultLocale;
const ogLocales: Record<string, string> = {
  en: "en_US",
  es: "es_ES",
  ca: "ca_ES",
};
const ogLocale = (code: string) =>
  ogLocales[code] ?? `${code}_${code.toUpperCase()}`;

interface Tag {
  name: string;
  attrs: Record<string, string>;
}

function tags(html: string, names: string[]): Tag[] {
  const out: Tag[] = [];
  const tagRe = new RegExp(`<(${names.join("|")})\\b([^>]*)>`, "gi");
  for (const m of html.matchAll(tagRe)) {
    const attrs: Record<string, string> = {};
    for (const a of m[2].matchAll(
      /([\w:-]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g,
    )) {
      attrs[a[1]] = a[2] ?? a[3] ?? a[4] ?? "";
    }
    out.push({ name: m[1].toLowerCase(), attrs });
  }
  return out;
}

const decode = (s: string) =>
  s
    .replace(/&quot;/g, '"')
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&#39;/g, "'");

interface Page {
  file: string;
  locale: string;
  /** Locale-free path, `/docs/install/`. */
  path: string;
  html: string;
}

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name);
    return statSync(full).isDirectory() ? walk(full) : [full];
  });
}

function loadPages(): Page[] {
  return walk(dist)
    .filter((f) => f.endsWith("/index.html"))
    .map((file) => {
      const rel = `/${dirname(file).slice(dist.length + 1)}/`
        .replace(/^\/\.\/$/, "/")
        .replace("//", "/");
      const first = rel.split("/")[1];
      const locale =
        codes.includes(first) && first !== defaultLocale
          ? first
          : defaultLocale;
      const path =
        locale === defaultLocale ? rel : rel.slice(locale.length + 1);
      return { file, locale, path, html: readFileSync(file, "utf8") };
    })
    .filter((p) => !p.path.startsWith("/404/") && !p.path.startsWith("/api/"));
}

const pages = loadPages();
const url = (locale: string, path: string) =>
  `${origin}${base}${locale === defaultLocale ? "" : `/${locale}`}${path}`;
const meta = (p: Page, key: "name" | "property", value: string) =>
  tags(p.html, ["meta"]).find((t) => t.attrs[key] === value)?.attrs.content;
const metas = (p: Page, key: "name" | "property", value: string) =>
  tags(p.html, ["meta"])
    .filter((t) => t.attrs[key] === value)
    .map((t) => decode(t.attrs.content));
const title = (p: Page) =>
  decode(/<title>([^<]*)<\/title>/.exec(p.html)?.[1] ?? "");

function hreflangs(p: Page): Record<string, string> {
  return Object.fromEntries(
    tags(p.html, ["link"])
      .filter((t) => t.attrs.rel === "alternate" && t.attrs.hreflang)
      .map((t) => [t.attrs.hreflang, t.attrs.href]),
  );
}

function jsonLd(p: Page): Record<string, any>[] {
  return [
    ...p.html.matchAll(
      /<script type="application\/ld\+json">([\s\S]*?)<\/script>/g,
    ),
  ].map((m) => JSON.parse(m[1]));
}

const kind = (p: Page) =>
  p.path === "/"
    ? "home"
    : p.path.startsWith("/docs/")
      ? "docs"
      : p.path.startsWith("/changelog/")
        ? "changelog"
        : "other";

describe("built pages", () => {
  test("every locale has the landing page, changelog and every docs page", () => {
    expect(pages.length).toBeGreaterThan(0);
    const byLocale = (code: string) =>
      new Set(pages.filter((p) => p.locale === code).map((p) => p.path));
    const reference = byLocale(defaultLocale);
    expect(reference.has("/")).toBe(true);
    expect(reference.has("/changelog/")).toBe(true);
    for (const code of codes) expect(byLocale(code)).toEqual(reference);
  });

  test("html lang matches the locale", () => {
    for (const p of pages) expect(p.html).toContain(`<html lang="${p.locale}"`);
  });

  test("unique title and description per locale, within search-result length", () => {
    for (const code of codes) {
      const own = pages.filter((p) => p.locale === code);
      const titles = own.map(title);
      const descriptions = own.map((p) =>
        decode(meta(p, "name", "description") ?? ""),
      );
      expect(new Set(titles).size).toBe(own.length);
      expect(new Set(descriptions).size).toBe(own.length);
      for (const t of titles) {
        expect(t.length).toBeGreaterThan(0);
        expect(t.length).toBeLessThanOrEqual(70);
      }
      for (const d of descriptions) {
        expect(d.length).toBeGreaterThanOrEqual(40);
        expect(d.length).toBeLessThanOrEqual(165);
      }
    }
  });

  test("canonical URL is absolute and points at the page itself", () => {
    for (const p of pages) {
      const links = tags(p.html, ["link"]).filter(
        (t) => t.attrs.rel === "canonical",
      );
      expect(links.length).toBe(1);
      expect(links[0].attrs.href).toBe(url(p.locale, p.path));
    }
  });

  test("hreflang lists every locale plus x-default and is reciprocal", () => {
    for (const p of pages) {
      const set = hreflangs(p);
      expect(Object.keys(set).sort()).toEqual([...codes, "x-default"].sort());
      expect(set["x-default"]).toBe(url(defaultLocale, p.path));
      for (const code of codes) expect(set[code]).toBe(url(code, p.path));
      expect(set[p.locale]).toBe(url(p.locale, p.path));
      for (const code of codes) {
        const target = pages.find(
          (q) => q.locale === code && q.path === p.path,
        );
        expect(target).toBeDefined();
        expect(hreflangs(target!)).toEqual(set);
      }
    }
  });

  test("Open Graph and Twitter card tags", () => {
    for (const p of pages) {
      const desc = meta(p, "name", "description");
      expect(meta(p, "property", "og:title")).toBe(
        meta(p, "name", "twitter:title"),
      );
      expect(title(p)).toBe(decode(meta(p, "property", "og:title") ?? ""));
      expect(meta(p, "property", "og:description")).toBe(desc);
      expect(meta(p, "name", "twitter:description")).toBe(desc);
      expect(meta(p, "property", "og:url")).toBe(url(p.locale, p.path));
      expect(meta(p, "property", "og:site_name")).toBe("Wren");
      expect(meta(p, "property", "og:type")).toBe(
        kind(p) === "docs" ? "article" : "website",
      );
      expect(meta(p, "property", "og:locale")).toBe(ogLocale(p.locale));
      expect(metas(p, "property", "og:locale:alternate").sort()).toEqual(
        codes
          .filter((c) => c !== p.locale)
          .map(ogLocale)
          .sort(),
      );
      expect(meta(p, "name", "twitter:card")).toBe("summary_large_image");
      const image = meta(p, "property", "og:image");
      expect(image).toBe(meta(p, "name", "twitter:image"));
      expect(image?.startsWith(`${origin}${base}/og/${p.locale}/`)).toBe(true);
      expect(meta(p, "property", "og:image:width")).toBe("1200");
      expect(meta(p, "property", "og:image:height")).toBe("630");
      expect(
        existsSync(join(dist, image!.slice(`${origin}${base}/`.length))),
      ).toBe(true);
    }
  });

  test("generated Open Graph images are 1200x630 PNGs", () => {
    for (const p of pages) {
      const image = meta(p, "property", "og:image")!;
      const png = readFileSync(
        join(dist, image.slice(`${origin}${base}/`.length)),
      );
      expect([...png.subarray(0, 8)]).toEqual([
        137, 80, 78, 71, 13, 10, 26, 10,
      ]);
      expect(png.readUInt32BE(16)).toBe(1200);
      expect(png.readUInt32BE(20)).toBe(630);
      expect(png.length).toBeLessThan(300_000);
    }
  });

  test("JSON-LD per page type", () => {
    for (const p of pages) {
      const types = jsonLd(p).map((d) => d["@type"]);
      for (const d of jsonLd(p))
        expect(d["@context"]).toBe("https://schema.org");
      if (kind(p) === "home") {
        expect(types).toContain("SoftwareApplication");
        expect(types).toContain("WebSite");
        const app = jsonLd(p).find(
          (d) => d["@type"] === "SoftwareApplication",
        )!;
        expect(app.name).toBe("Wren");
        expect(app.inLanguage).toBe(p.locale);
      }
      if (kind(p) === "docs") {
        expect(types).toContain("TechArticle");
        expect(types).toContain("BreadcrumbList");
        const article = jsonLd(p).find((d) => d["@type"] === "TechArticle")!;
        expect(article.url).toBe(url(p.locale, p.path));
        expect(article.inLanguage).toBe(p.locale);
        const crumbs = jsonLd(p).find((d) => d["@type"] === "BreadcrumbList")!;
        expect(crumbs.itemListElement.at(-1).item).toBe(url(p.locale, p.path));
      }
      if (kind(p) === "changelog") expect(types).toContain("BreadcrumbList");
    }
  });

  test("changelog advertises its Atom feed", () => {
    for (const p of pages.filter((q) => kind(q) === "changelog")) {
      const feed = tags(p.html, ["link"]).find(
        (t) => t.attrs.type === "application/atom+xml",
      );
      expect(feed?.attrs.href).toBe(
        `${base}${p.locale === defaultLocale ? "" : `/${p.locale}`}/changelog/atom.xml`,
      );
    }
  });
});

describe("site files", () => {
  test("sitemap lists every page with xhtml alternates for every locale and x-default", () => {
    const xml = readFileSync(join(dist, "sitemap.xml"), "utf8");
    expect(xml).toContain('xmlns:xhtml="http://www.w3.org/1999/xhtml"');
    for (const p of pages) {
      const loc = `<loc>${url(p.locale, p.path)}</loc>`;
      const block = xml.split("<url>").find((b) => b.includes(loc));
      expect(block).toBeDefined();
      for (const code of codes) {
        expect(block).toContain(
          `hreflang="${code}" href="${url(code, p.path)}"`,
        );
      }
      expect(block).toContain(
        `hreflang="x-default" href="${url(defaultLocale, p.path)}"`,
      );
    }
  });

  test("robots.txt allows crawling and points at the sitemap", () => {
    const robots = readFileSync(join(dist, "robots.txt"), "utf8");
    expect(robots).toContain("User-agent: *");
    expect(robots).toContain(`Sitemap: ${origin}${base}/sitemap.xml`);
  });

  test("a 404 page per locale, kept out of the index", () => {
    for (const code of codes) {
      const file =
        code === defaultLocale ? "404.html" : `${code}/404/index.html`;
      const html = readFileSync(join(dist, file), "utf8");
      expect(html).toContain(`<html lang="${code}"`);
      expect(html).toContain('name="robots" content="noindex"');
      expect(html).not.toContain('rel="canonical"');
      expect(html).not.toMatch(/rel="alternate"[^>]*hreflang/);
    }
  });

  test("Atom feed per locale", () => {
    for (const code of codes) {
      const xml = readFileSync(
        join(dist, code === defaultLocale ? "" : code, "changelog/atom.xml"),
        "utf8",
      );
      expect(xml).toContain('<feed xmlns="http://www.w3.org/2005/Atom"');
      expect(xml).toContain(`xml:lang="${code}"`);
      expect(xml).toContain(`<id>${url(code, "/changelog/atom.xml")}</id>`);
      expect(xml).toContain("<updated>");
    }
  });
});

// Budgets: gzip bytes of what a page loads. Keep in step with lighthouserc.cjs.
const budget = { staticJs: 0, docsJs: 230_000, css: 20_000 };

function assetBytes(file: string): number {
  return gzipSync(readFileSync(file)).length;
}

/** Local files a page loads as JS or CSS, following static imports of JS modules. */
function loads(p: Page): { js: Set<string>; css: Set<string> } {
  const js = new Set<string>();
  const css = new Set<string>();
  const local = (href: string) =>
    href.startsWith(`${base}/`)
      ? join(dist, href.slice(base.length + 1))
      : undefined;
  const addJs = (file: string | undefined) => {
    if (!file || js.has(file)) return;
    js.add(file);
    const src = readFileSync(file, "utf8");
    for (const m of src.matchAll(
      /(?:from|import)\s*["']\.\/([^"']+\.js)["']/g,
    )) {
      addJs(join(dirname(file), m[1]));
    }
  };
  for (const t of tags(p.html, ["script", "link"])) {
    if (t.name === "script" && t.attrs.src) addJs(local(t.attrs.src));
    if (t.name === "link" && t.attrs.rel === "modulepreload")
      addJs(local(t.attrs.href));
    if (t.name === "link" && t.attrs.rel === "stylesheet") {
      const f = local(t.attrs.href);
      if (f) css.add(f);
    }
  }
  for (const island of tags(p.html, ["astro-island"])) {
    for (const key of ["component-url", "renderer-url"]) {
      addJs(local(island.attrs[key] ?? ""));
    }
  }
  return { js, css };
}

describe("byte budgets (gzip)", () => {
  test("static pages ship no JavaScript, docs stay inside the island budget, CSS stays small", () => {
    for (const p of pages) {
      const { js, css } = loads(p);
      const jsBytes = [...js].reduce((n, f) => n + assetBytes(f), 0);
      // CSS inlined by Astro counts too.
      const inline = [
        ...p.html.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g),
      ].reduce((n, m) => n + gzipSync(m[1]).length, 0);
      const cssBytes = [...css].reduce((n, f) => n + assetBytes(f), 0) + inline;
      const label = `${p.locale}${p.path}`;
      if (kind(p) === "docs") {
        expect({ label, bytes: jsBytes }).toEqual({
          label,
          bytes: Math.min(jsBytes, budget.docsJs),
        });
      } else {
        expect({
          label,
          bytes: jsBytes,
          islands: p.html.includes("<astro-island"),
        }).toEqual({
          label,
          bytes: budget.staticJs,
          islands: false,
        });
      }
      expect({ label, bytes: cssBytes }).toEqual({
        label,
        bytes: Math.min(cssBytes, budget.css),
      });
    }
  });

  test("fonts are self-hosted, preloaded and swap in", () => {
    for (const p of pages) {
      const preload = tags(p.html, ["link"]).find(
        (t) => t.attrs.rel === "preload" && t.attrs.as === "font",
      );
      expect(preload?.attrs.crossorigin).toBeDefined();
      expect(
        existsSync(join(dist, preload!.attrs.href.slice(base.length + 1))),
      ).toBe(true);
    }
    const css = walk(join(dist, "_astro"))
      .filter((f) => f.endsWith(".css"))
      .map((f) => readFileSync(f, "utf8"))
      .join("\n");
    expect(css).toContain("font-display:swap");
    expect(css).not.toMatch(/fonts\.(googleapis|gstatic)\.com/);
    expect(
      walk(join(dist, "_astro")).filter((f) =>
        /cyrillic|greek|vietnamese/.test(f),
      ),
    ).toEqual([]);
  });
});
