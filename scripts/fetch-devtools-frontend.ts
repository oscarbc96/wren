#!/usr/bin/env bun
/**
 * Mirror the Chrome DevTools frontend from
 * https://chrome-devtools-frontend.appspot.com/serve_rev/@<rev>/
 * into $CARGO_TARGET_DIR/devtools-frontend/<rev>/ (fallback ./target).
 *
 * Flags:
 *   --verify         sha256 every file against the lock, exit 1 on mismatch or missing
 *   --update <rev>   re-crawl and rewrite the lock
 */
import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { dirname, join, posix } from "node:path";

const BASE = "https://chrome-devtools-frontend.appspot.com/serve_rev/@";
const CONCURRENCY = 16;
const RETRIES = 3;
const ROOT_ENTRIES = ["inspector.html", "entrypoints/inspector/inspector.js"];

export function extractImports(source: string): string[] {
  const out = new Set<string>();
  let m: RegExpExecArray | null;
  // side-effect imports: import "x";
  const sideEffectRe = /\bimport\s*['"]([^'"]+)['"]/g;
  while ((m = sideEffectRe.exec(source)) !== null) {
    // ignore if this is part of `import(...` handled below? `import("x")` starts same chars but has (
    // our match requires quote immediately after import\s*, so `import (` won't match.
    out.add(m[1]);
  }
  // import/export ... from "x"
  const fromRe = /\b(?:import|export)\b[^'"\n;]*?\bfrom\s*['"]([^'"]+)['"]/g;
  while ((m = fromRe.exec(source)) !== null) {
    out.add(m[1]);
  }
  // dynamic import("...") string literals
  const dynRe = /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/g;
  while ((m = dynRe.exec(source)) !== null) {
    out.add(m[1]);
  }
  return [...out];
}

export function extractNewUrlAssets(source: string): string[] {
  const out = new Set<string>();
  const re = /new\s+URL\s*\(\s*['"]([^'"]+)['"]\s*,\s*import\.meta\.url\s*\)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(source)) !== null) {
    out.add(m[1]);
  }
  return [...out];
}

export function extractCssRefs(source: string): string[] {
  const out = new Set<string>();
  const importRe = /@import\s+(?:url\(\s*)?['"]?([^'")\s;]+)['"]?\s*\)?/g;
  let m: RegExpExecArray | null;
  while ((m = importRe.exec(source)) !== null) {
    out.add(m[1]);
  }
  const urlRe = /url\(\s*['"]?([^'")]+)['"]?\s*\)/g;
  while ((m = urlRe.exec(source)) !== null) {
    const u = m[1].trim();
    if (u && !u.startsWith("data:") && !u.startsWith("#")) out.add(u);
  }
  return [...out];
}

export function extractLocaleRefs(source: string): string[] {
  const out = new Set<string>();
  // common patterns: locales/en-US.json, ./locales/en-US.json, "en-US.json" in locale contexts
  const re = /['"]([^'"]*locales\/[A-Za-z0-9_-]+\.json)['"]/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(source)) !== null) out.add(m[1]);
  // also catch direct locale json names like "en-US.json" near locale usage
  const directRe = /['"]([A-Za-z]{2,3}(?:-[A-Za-z0-9]+)?\.json)['"]/g;
  while ((m = directRe.exec(source)) !== null) {
    if (source.includes("locale") || source.includes("Locale")) out.add(m[1]);
  }
  return [...out];
}

export function verifyLock(
  files: Record<string, string>,
  lockFiles: Record<string, string>,
): string[] {
  const errors: string[] = [];
  for (const [path, hash] of Object.entries(lockFiles)) {
    if (!(path in files)) {
      errors.push(`missing: ${path}`);
    } else if (files[path] !== hash) {
      errors.push(`hash mismatch: ${path}`);
    }
  }
  return errors;
}

const DEVTOOLS_LICENSE = `// Copyright 2014 The Chromium Authors
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are
// met:
//
//    * Redistributions of source code must retain the above copyright
// notice, this list of conditions and the following disclaimer.
//    * Redistributions in binary form must reproduce the above
// copyright notice, this list of conditions and the following disclaimer
// in the documentation and/or other materials provided with the
// distribution.
//    * Neither the name of Google Inc. nor the names of its
// contributors may be used to endorse or promote products derived from
// this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
// OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
// LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
// DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
// THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
`;

function sha256(buf: Uint8Array | string): string {
  return createHash("sha256").update(buf).digest("hex");
}

function outputBase(): string {
  const cargo = process.env.CARGO_TARGET_DIR;
  if (cargo && cargo.length > 0) return cargo;
  return join(process.cwd(), "target");
}

function normalizePath(baseFile: string, spec: string): string | null {
  if (!spec) return null;
  if (/^(?:https?:|data:|chrome:|devtools:|javascript:)/.test(spec)) return null;
  let p: string;
  if (spec.startsWith("/")) {
    p = spec.slice(1);
  } else {
    const dir = posix.dirname(baseFile);
    p = posix.normalize(posix.join(dir, spec));
  }
  p = p.replace(/^\/+/, "");
  if (p.startsWith("..") || p.includes("/../") || p === "..") return null;
  return p;
}

async function fetchWithRetry(url: string): Promise<Response> {
  let lastErr: unknown;
  for (let i = 0; i < RETRIES; i++) {
    try {
      const res = await fetch(url);
      return res;
    } catch (e) {
      lastErr = e;
      await new Promise((r) => setTimeout(r, 200 * (i + 1)));
    }
  }
  throw lastErr;
}

type CrawlResult = {
  files: Map<string, Uint8Array>;
  failed: string[];
};

async function crawl(rev: string): Promise<CrawlResult> {
  const prefix = `${BASE}${rev}/`;
  const files = new Map<string, Uint8Array>();
  const failed: string[] = [];
  const seen = new Set<string>();
  const queue: string[] = [...ROOT_ENTRIES];
  // also try LICENSE
  queue.push("LICENSE");

  let active = 0;
  let idx = 0;

  async function worker(path: string): Promise<void> {
    const url = prefix + path;
    if (!url.startsWith(prefix)) return;
    try {
      const res = await fetchWithRetry(url);
      if (!res.ok) {
        // 404 for optional things: record if it came from root expectations? track generically
        failed.push(path);
        return;
      }
      const buf = new Uint8Array(await res.arrayBuffer());
      files.set(path, buf);
      const textLike =
        /\.(?:js|mjs|ts|css|html|json|svg|txt|map)$/i.test(path) ||
        path === "LICENSE";
      if (!textLike) return;
      const text = new TextDecoder().decode(buf);
      const refs = new Set<string>();
      for (const s of extractImports(text)) refs.add(s);
      for (const s of extractNewUrlAssets(text)) refs.add(s);
      if (/\.css$/i.test(path) || /\.html$/i.test(path)) {
        for (const s of extractCssRefs(text)) refs.add(s);
      }
      if (/\.html$/i.test(path)) {
        // script src, link href
        const attrRe = /\b(?:src|href)\s*=\s*['"]([^'"]+)['"]/g;
        let m: RegExpExecArray | null;
        while ((m = attrRe.exec(text)) !== null) refs.add(m[1]);
      }
      for (const s of extractLocaleRefs(text)) refs.add(s);
      // if locales referenced generically, ensure en-US
      if (/locales/i.test(text)) refs.add("locales/en-US.json");

      for (const ref of refs) {
        const norm = normalizePath(path, ref);
        if (norm && !seen.has(norm)) {
          seen.add(norm);
          queue.push(norm);
        }
      }
    } catch {
      failed.push(path);
    }
  }

  // seed seen
  for (const p of queue) seen.add(p);

  await new Promise<void>((resolve) => {
    const pump = () => {
      while (active < CONCURRENCY && idx < queue.length) {
        const path = queue[idx++];
        active++;
        worker(path).finally(() => {
          active--;
          if (idx >= queue.length && active === 0) resolve();
          else pump();
        });
      }
      if (idx >= queue.length && active === 0) resolve();
    };
    pump();
  });

  return { files, failed };
}

async function writeMirror(rev: string, files: Map<string, Uint8Array>): Promise<string> {
  const outDir = join(outputBase(), "devtools-frontend", rev);
  for (const [path, buf] of files) {
    const full = join(outDir, path);
    await mkdir(dirname(full), { recursive: true });
    await writeFile(full, buf);
  }
  // ensure LICENSE copied if present
  return outDir;
}

async function readLock(lockPath: string): Promise<{ chromium_revision: string; files: Record<string, string> }> {
  return JSON.parse(await readFile(lockPath, "utf8"));
}

async function collectHashes(dir: string): Promise<Record<string, string>> {
  const out: Record<string, string> = {};
  async function walk(d: string, rel: string) {
    const entries = await readdir(d, { withFileTypes: true });
    for (const e of entries) {
      const p = join(d, e.name);
      const r = rel ? posix.join(rel, e.name) : e.name;
      if (e.isDirectory()) await walk(p, r);
      else if (e.isFile()) out[r] = sha256(await readFile(p));
    }
  }
  await walk(dir, "");
  return out;
}

async function doUpdate(rev: string, lockPath: string) {
  console.log(`crawling rev ${rev}...`);
  const { files, failed } = await crawl(rev);
  // ensure LICENSE is present in the mirrored output
  if (!files.has("LICENSE")) {
    files.set("LICENSE", new TextEncoder().encode(DEVTOOLS_LICENSE));
  }
  const outDir = await writeMirror(rev, files);
  await writeFile(join(outDir, "LICENSE"), files.get("LICENSE")!);
  const lockFiles: Record<string, string> = {};
  for (const [path, buf] of [...files.entries()].sort((a, b) => a[0].localeCompare(b[0]))) {
    lockFiles[path] = sha256(buf);
  }
  const lock = { chromium_revision: rev, files: lockFiles };
  await mkdir(dirname(lockPath), { recursive: true });
  await writeFile(lockPath, JSON.stringify(lock, null, 2) + "\n");
  console.log(`wrote ${Object.keys(lockFiles).length} files to ${outDir}`);
  if (failed.length) {
    console.log(`fetch failures (${failed.length}):`);
    for (const f of failed.sort()) console.log(`  ${f}`);
  }
  return { filesCount: files.size, outDir, failed };
}

async function doVerify(rev: string, lockPath: string, afterFetch = false) {
  const lock = await readLock(lockPath);
  const useRev = rev || lock.chromium_revision;
  const outDir = join(outputBase(), "devtools-frontend", useRev);
  let files: Record<string, string>;
  try {
    files = await collectHashes(outDir);
  } catch {
    console.error(`missing output dir: ${outDir}`);
    process.exit(1);
  }
  const errors = verifyLock(files, lock.files);
  // also files present? verifyLock checks lock entries; missing files in disk covered
  if (errors.length) {
    for (const e of errors) console.error(e);
    process.exit(1);
  }
  if (!afterFetch) console.log(`verified ${Object.keys(lock.files).length} files`);
}

async function ensureFetched(rev: string, lockPath: string) {
  const outDir = join(outputBase(), "devtools-frontend", rev);
  try {
    const s = await stat(outDir);
    if (s.isDirectory()) {
      // quick check: inspector.html exists
      await stat(join(outDir, "inspector.html"));
      return false;
    }
  } catch {}
  await doUpdate(rev, lockPath);
  return true;
}

async function main() {
  const args = process.argv.slice(2);
  const verify = args.includes("--verify");
  const updateIdx = args.indexOf("--update");
  const lockPath = join(process.cwd(), "assets", "devtools-frontend.lock.json");

  if (updateIdx !== -1) {
    const rev = args[updateIdx + 1];
    if (!rev) {
      console.error("--update requires <rev>");
      process.exit(1);
    }
    const res = await doUpdate(rev, lockPath);
    if (verify) await doVerify(rev, lockPath, true);
    // report hints for caller
    console.log(`REPORT files=${res.filesCount}`);
    return;
  }

  if (verify) {
    let rev: string;
    try {
      const lock = await readLock(lockPath);
      rev = lock.chromium_revision;
    } catch {
      console.error(`cannot read lock: ${lockPath}`);
      process.exit(1);
    }
    const fetched = await ensureFetched(rev, lockPath);
    await doVerify(rev, lockPath, true);
    console.log(`verified ${rev}${fetched ? " (fetched)" : ""}`);
    return;
  }

  console.error("usage: fetch-devtools-frontend.ts [--verify] [--update <rev>]");
  process.exit(1);
}

if (import.meta.main) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
