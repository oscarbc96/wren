import type { APIRoute, GetStaticPaths, InferGetStaticPropsType } from "astro";
import { nonDefaultLocales } from "@/i18n";
import { docsLlms, source } from "@/lib/source";

export const prerender = true;

export const getStaticPaths = (() =>
  nonDefaultLocales.flatMap((locale) =>
    source.getPages(locale).map((page) => ({
      params: { locale, slug: decodeURI(page.slugs.join("/")) || "index" },
      props: { page },
    })),
  )) satisfies GetStaticPaths;

export const GET: APIRoute<
  InferGetStaticPropsType<typeof getStaticPaths>
> = async ({ props: { page } }) =>
  new Response(await docsLlms.page(page), {
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });
