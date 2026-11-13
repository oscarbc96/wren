/**
 * Serves `dist/` the way GitHub Pages does: under the `/wren` base, directory
 * indexes, the root `404.html` for misses, gzip for text and long-lived caching
 * for fingerprinted assets. Used by Lighthouse CI.
 *
 * Run from `site/`: `bun scripts/serve-dist.ts [port]` (default 4173).
 */
import { existsSync, statSync } from "node:fs";
import { extname, join, normalize } from "node:path";

const port = Number(process.argv[2] ?? process.env.PORT ?? 4173);
const base = "/wren";
const root = join(process.cwd(), "dist");
const compressible = new Set([
  ".html",
  ".css",
  ".js",
  ".json",
  ".xml",
  ".txt",
  ".md",
  ".svg",
]);

function resolveFile(pathname: string): string | undefined {
  if (!pathname.startsWith(`${base}/`)) return undefined;
  const rel = normalize(decodeURIComponent(pathname.slice(base.length)));
  if (rel.includes("..")) return undefined;
  let file = join(root, rel);
  if (pathname.endsWith("/")) file = join(file, "index.html");
  return existsSync(file) && statSync(file).isFile() ? file : undefined;
}

Bun.serve({
  port,
  fetch(req) {
    const url = new URL(req.url);
    const file = resolveFile(url.pathname);
    const status = file ? 200 : 404;
    const target = file ?? join(root, "404.html");
    const ext = extname(target);
    const headers = new Headers();
    headers.set(
      "cache-control",
      url.pathname.includes("/_astro/")
        ? "public, max-age=31536000, immutable"
        : "public, max-age=600",
    );
    const body = Bun.file(target);
    if (
      compressible.has(ext) &&
      req.headers.get("accept-encoding")?.includes("gzip")
    ) {
      return body.arrayBuffer().then((buf) => {
        headers.set("content-encoding", "gzip");
        headers.set("content-type", body.type);
        headers.set("vary", "accept-encoding");
        return new Response(Bun.gzipSync(new Uint8Array(buf)), {
          status,
          headers,
        });
      });
    }
    return new Response(body, { status, headers });
  },
});
console.log(`serving dist at http://localhost:${port}${base}/`);
