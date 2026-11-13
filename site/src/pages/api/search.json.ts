import type { APIRoute } from "astro";
import { createFromSource } from "fumadocs-core/search/server";
import { source } from "@/lib/source";

export const prerender = true;

/** Fumadocs picks a tokenizer per locale, `multilingual` unless mapped. */
const server = createFromSource(source, {
  localeMap: { en: "english", es: "spanish" },
});

export const GET: APIRoute = () => server.staticGET();
