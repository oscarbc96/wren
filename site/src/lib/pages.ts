import { localeCodes, getUi } from "@/i18n";
import { source } from "@/lib/source";
import { getReleasePages } from "@/lib/github";

/** A page of the site, in one locale. `path` is locale-free and ends in `/`. */
export interface SitePage {
  locale: string;
  path: string;
  /** Key of the generated Open Graph image: `<locale>/<key>.png`. */
  ogKey: string;
  title: string;
  description: string;
  kind: "home" | "changelog" | "downloads" | "docs";
}

/** Locale-free path of a docs page. */
export function docsPath(slugs: string[]): string {
  return slugs.length > 0 ? `/docs/${slugs.join("/")}/` : "/docs/";
}

/** Open Graph key of a docs page. */
export function docsOgKey(slugs: string[]): string {
  return slugs.length > 0 ? `docs/${slugs.join("/")}` : "docs";
}

/** Every page that has its own Open Graph image and sitemap entry (first changelog page only). */
export function getSitePages(): SitePage[] {
  return localeCodes.flatMap((locale) => {
    const ui = getUi(locale);
    const pages: SitePage[] = [
      {
        locale,
        path: "/",
        ogKey: "home",
        title: ui.home.title,
        description: ui.home.description,
        kind: "home",
      },
      {
        locale,
        path: "/changelog/",
        ogKey: "changelog",
        title: ui.changelog.title,
        description: ui.changelog.description,
        kind: "changelog",
      },
      {
        locale,
        path: "/downloads/",
        ogKey: "downloads",
        title: ui.downloads.title,
        description: ui.downloads.description,
        kind: "downloads",
      },
    ];
    for (const page of source.getPages(locale)) {
      pages.push({
        locale,
        path: docsPath(page.slugs),
        ogKey: docsOgKey(page.slugs),
        title: page.data.title,
        description: page.data.description ?? "",
        kind: "docs",
      });
    }
    return pages;
  });
}

/** Locale-free paths of the changelog pages (`/changelog/`, `/changelog/2/`, ...). */
export async function getChangelogPaths(): Promise<string[]> {
  const total = Math.max((await getReleasePages()).length, 1);
  return Array.from({ length: total }, (_, i) =>
    i === 0 ? "/changelog/" : `/changelog/${i + 1}/`,
  );
}
