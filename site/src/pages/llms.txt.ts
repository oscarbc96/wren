import type { APIRoute } from "astro";
import { defaultLocale } from "@/i18n";
import { docsLlms } from "@/lib/source";

export const prerender = true;

export const GET: APIRoute = () =>
  new Response(docsLlms.index(defaultLocale), {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
