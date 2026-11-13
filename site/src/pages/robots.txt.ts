import type { APIRoute } from "astro";
import { buildRobots } from "@/lib/feeds";

export const prerender = true;

export const GET: APIRoute = () =>
  new Response(buildRobots(), {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
