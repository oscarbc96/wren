import type { APIRoute, GetStaticPaths } from "astro";
import { nonDefaultLocales } from "@/i18n";
import { docsLlms } from "@/lib/source";

export const prerender = true;

export const getStaticPaths = (() =>
  nonDefaultLocales.map((locale) => ({
    params: { locale },
  }))) satisfies GetStaticPaths;

export const GET: APIRoute = async ({ params }) =>
  new Response(await docsLlms.full(params.locale), {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
