import type { APIRoute, GetStaticPaths } from "astro";
import { renderOgImage } from "@/lib/og";
import { getSitePages } from "@/lib/pages";

export const prerender = true;

export const getStaticPaths = (() =>
  getSitePages().map((page) => ({
    params: { locale: page.locale, key: page.ogKey },
    props: { page },
  }))) satisfies GetStaticPaths;

export const GET: APIRoute = async ({ props }) => {
  const { page } = props as { page: ReturnType<typeof getSitePages>[number] };
  const png = await renderOgImage(page);
  return new Response(new Uint8Array(png), {
    headers: { "Content-Type": "image/png" },
  });
};
