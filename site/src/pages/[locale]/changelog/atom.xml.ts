import type { APIRoute, GetStaticPaths } from "astro";
import { nonDefaultLocales } from "@/i18n";
import { buildChangelogFeed } from "@/lib/feeds";

export const prerender = true;

export const getStaticPaths = (() =>
  nonDefaultLocales.map((locale) => ({
    params: { locale },
  }))) satisfies GetStaticPaths;

export const GET: APIRoute = async ({ params }) =>
  new Response(await buildChangelogFeed(params.locale as string), {
    headers: { "Content-Type": "application/atom+xml; charset=utf-8" },
  });
