import type { APIRoute } from "astro";
import { defaultLocale } from "@/i18n";
import { buildChangelogFeed } from "@/lib/feeds";

export const prerender = true;

export const GET: APIRoute = async () =>
  new Response(await buildChangelogFeed(defaultLocale), {
    headers: { "Content-Type": "application/atom+xml; charset=utf-8" },
  });
