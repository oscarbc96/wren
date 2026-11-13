import { getCollection } from "astro:content";
import { llms, loader } from "fumadocs-core/source";
import { toFumadocsSource } from "@fumadocs/astro/source";
import { defaultLocale, i18n, localePath } from "@/i18n";

export const source = loader({
  source: toFumadocsSource(
    await getCollection("docs"),
    await getCollection("meta"),
  ),
  baseUrl: localePath(defaultLocale, "/docs"),
  i18n,
  url: (slugs, locale) =>
    localePath(
      locale ?? i18n.defaultLanguage,
      `/docs/${slugs.join("/")}`,
    ).replace(/\/$/, "") || "/",
});

/** URL of the Markdown form of a docs page, served next to its HTML page. */
export function getPageMarkdownUrl(page: { slugs: string[]; locale?: string }) {
  const segments = page.slugs.length > 0 ? [...page.slugs] : ["index"];
  segments[segments.length - 1] += ".md";
  return localePath(
    page.locale ?? defaultLocale,
    `/docs/${segments.join("/")}`,
  );
}

export const docsLlms = llms(source, {
  renderUrl: (item, { lang }) => {
    const page = source.getNodePage(item, lang);
    return page ? getPageMarkdownUrl(page) : item.url;
  },
  renderPage: async (page) => `# ${page.data.title} (${page.url})

${page.data.description ? `${page.data.description}\n\n` : ""}${await page.data.getText("processed")}`,
});
