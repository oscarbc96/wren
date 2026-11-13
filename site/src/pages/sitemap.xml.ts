import type { APIRoute } from "astro";
import { localeCodes } from "@/i18n";
import { buildSitemap } from "@/lib/feeds";
import { getChangelogPaths, getSitePages } from "@/lib/pages";

export const prerender = true;

export const GET: APIRoute = async () => {
  const changelog = await getChangelogPaths();
  const paths = [
    ...new Set([...getSitePages().map((page) => page.path), ...changelog]),
  ];
  return new Response(buildSitemap(paths, localeCodes), {
    headers: { "Content-Type": "application/xml; charset=utf-8" },
  });
};
