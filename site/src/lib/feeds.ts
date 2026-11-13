import { getUi } from "@/i18n";
import { getReleasePages } from "@/lib/github";
import { absoluteUrl, alternatesFor, siteUrl } from "@/lib/seo";

export function xmlEscape(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Atom feed of the GitHub Releases, with the chrome text in `locale` (notes stay English). */
export async function buildChangelogFeed(locale: string): Promise<string> {
  const t = getUi(locale).changelog;
  const entries = (await getReleasePages()).flat();
  const page = absoluteUrl(locale, "/changelog/");
  const self = absoluteUrl(locale, "/changelog/atom.xml");
  const updated = entries[0]?.date ?? new Date();
  const items = entries
    .map(
      (entry) => `  <entry>
    <id>${xmlEscape(entry.url ?? `${page}#${entry.id}`)}</id>
    <title>${xmlEscape(entry.title)}</title>
    <updated>${entry.date.toISOString()}</updated>
${entry.url ? `    <link rel="alternate" type="text/html" href="${xmlEscape(entry.url)}"/>\n` : ""}${entry.content ? `    <content type="text">${xmlEscape(entry.content)}</content>\n` : ""}  </entry>`,
    )
    .join("\n");
  return `<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xml:lang="${locale}">
  <id>${self}</id>
  <title>${xmlEscape(t.title)}</title>
  <subtitle>${xmlEscape(t.description)}</subtitle>
  <updated>${updated.toISOString()}</updated>
  <link rel="self" type="application/atom+xml" href="${self}"/>
  <link rel="alternate" type="text/html" href="${page}"/>
${items}
</feed>
`;
}

/** sitemap.xml with an xhtml:link alternate set (every locale plus x-default) on each URL. */
export function buildSitemap(paths: string[], locales: string[]): string {
  const urls = paths
    .flatMap((path) =>
      locales.map((locale) => {
        const links = alternatesFor(path)
          .map(
            (a) =>
              `    <xhtml:link rel="alternate" hreflang="${a.hreflang}" href="${xmlEscape(a.href)}"/>`,
          )
          .join("\n");
        return `  <url>\n    <loc>${xmlEscape(absoluteUrl(locale, path))}</loc>\n${links}\n  </url>`;
      }),
    )
    .join("\n");
  return `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${urls}
</urlset>
`;
}

export function buildRobots(): string {
  return `User-agent: *
Allow: /

Sitemap: ${siteUrl("/sitemap.xml")}
`;
}
