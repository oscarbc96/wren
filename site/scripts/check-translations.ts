/**
 * Keeps every locale in step with English.
 *
 * 1. Parity (full tree): every English docs page has a file per locale and no
 *    locale has an orphan; meta files list the same pages; UI string tables
 *    have identical key sets.
 * 2. Change sync (diff based): if an English docs page, meta file or UI table
 *    changed, the same entry for every locale must be in the change set too.
 *    A `Translations-Unchanged: <reason>` line in a commit message or PR body
 *    skips this rule (typo or formatting-only English edits).
 *
 * Locales come from `src/i18n/config.json`. Run from `site/`:
 *   bun scripts/check-translations.ts                 parity only
 *   bun scripts/check-translations.ts --staged        parity + staged files
 *   bun scripts/check-translations.ts --base <sha>    parity + <sha>...HEAD
 */
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

export interface Locales {
  defaultLocale: string;
  /** Non-default locale codes. */
  others: string[];
}

const DOCS = "content/docs";
const I18N = "src/i18n";
const ESCAPE = /^[ \t]*Translations-Unchanged:[ \t]*(\S.*)$/m;

export function loadLocales(root: string): Locales {
  const cfg = JSON.parse(readFileSync(join(root, I18N, "config.json"), "utf8"));
  const codes: string[] = cfg.locales.map((l: { code: string }) => l.code);
  return {
    defaultLocale: cfg.defaultLocale,
    others: codes.filter((c) => c !== cfg.defaultLocale),
  };
}

/** Docs page names present for `locale` (`undefined` = English). */
function pages(root: string, loc: Locales, locale?: string): string[] {
  const suffix = locale ? `.${locale}.mdx` : ".mdx";
  const out: string[] = [];
  for (const f of readdirSync(join(root, DOCS))) {
    if (!f.endsWith(suffix)) continue;
    const name = f.slice(0, -suffix.length);
    // English files must not look like a translation (`page.es.mdx`).
    if (!locale && loc.others.some((o) => name.endsWith(`.${o}`))) continue;
    out.push(name);
  }
  return out.sort();
}

function metaPages(root: string, locale?: string): string[] {
  const file = join(root, DOCS, locale ? `meta.${locale}.json` : "meta.json");
  if (!existsSync(file)) return [];
  return [...JSON.parse(readFileSync(file, "utf8")).pages].sort();
}

/** Dotted key paths of a nested value (arrays use their index). */
export function flattenKeys(value: unknown, prefix = ""): string[] {
  if (value === null || typeof value !== "object") return [prefix];
  return Object.entries(value).flatMap(([k, v]) =>
    flattenKeys(v, prefix ? `${prefix}.${k}` : k),
  );
}

async function uiKeys(root: string, locale: string): Promise<string[] | null> {
  const file = resolve(root, I18N, `${locale}.ts`);
  if (!existsSync(file)) return null;
  const table = (await import(file))[locale];
  if (table === undefined) return null;
  // `translations` maps Fumadocs' English text to a translation; English
  // relies on the built-in defaults, so it is a free-form record, not a schema.
  const { translations: _free, ...schema } = table;
  return flattenKeys(schema).sort();
}

function diff(a: string[], b: string[]) {
  const sb = new Set(b);
  const sa = new Set(a);
  return {
    missing: a.filter((x) => !sb.has(x)),
    extra: b.filter((x) => !sa.has(x)),
  };
}

/** Rule 1. Returns one message per problem. */
export async function checkParity(root: string): Promise<string[]> {
  const loc = loadLocales(root);
  const errors: string[] = [];
  const en = pages(root, loc);
  const enMeta = metaPages(root);
  const enUi = await uiKeys(root, loc.defaultLocale);
  if (!enUi)
    errors.push(`${I18N}/${loc.defaultLocale}.ts: missing or has no export`);

  for (const l of loc.others) {
    const { missing, extra } = diff(en, pages(root, loc, l));
    for (const p of missing)
      errors.push(`${DOCS}/${p}.${l}.mdx: missing translation`);
    for (const p of extra)
      errors.push(`${DOCS}/${p}.${l}.mdx: orphan, no English ${p}.mdx`);

    if (!existsSync(join(root, DOCS, `meta.${l}.json`))) {
      errors.push(`${DOCS}/meta.${l}.json: missing`);
    } else {
      const m = diff(enMeta, metaPages(root, l));
      for (const p of m.missing)
        errors.push(`${DOCS}/meta.${l}.json: missing page "${p}"`);
      for (const p of m.extra)
        errors.push(`${DOCS}/meta.${l}.json: unknown page "${p}"`);
    }

    const keys = await uiKeys(root, l);
    if (!keys) errors.push(`${I18N}/${l}.ts: missing or has no export`);
    else if (enUi) {
      const k = diff(enUi, keys);
      for (const x of k.missing)
        errors.push(`${I18N}/${l}.ts: missing key "${x}"`);
      for (const x of k.extra)
        errors.push(`${I18N}/${l}.ts: unknown key "${x}"`);
    }
  }
  return errors;
}

/** Extracts the `Translations-Unchanged` reason, or `null` if absent or empty. */
export function escapeReason(text: string): string | null {
  const m = ESCAPE.exec(text);
  return m ? m[1].trim() : null;
}

/** Rule 2. `changed` holds paths relative to the site root. */
export function checkChangeSync(
  root: string,
  changed: string[],
  message = "",
): string[] {
  if (escapeReason(message)) return [];
  const loc = loadLocales(root);
  const set = new Set(changed);
  const errors: string[] = [];
  const need = (trigger: string, siblings: string[]) => {
    for (const s of siblings) {
      if (!set.has(s)) errors.push(`${trigger} changed but ${s} did not`);
    }
  };

  for (const f of changed) {
    if (f === `${DOCS}/meta.json`) {
      need(
        f,
        loc.others.map((l) => `${DOCS}/meta.${l}.json`),
      );
    } else if (f === `${I18N}/${loc.defaultLocale}.ts`) {
      need(
        f,
        loc.others.map((l) => `${I18N}/${l}.ts`),
      );
    } else if (f.startsWith(`${DOCS}/`) && f.endsWith(".mdx")) {
      const name = f.slice(DOCS.length + 1, -".mdx".length);
      if (loc.others.some((o) => name.endsWith(`.${o}`))) continue;
      need(
        f,
        loc.others.map((l) => `${DOCS}/${name}.${l}.mdx`),
      );
    }
  }
  return errors;
}

function git(args: string[]): string {
  const p = Bun.spawnSync(["git", ...args]);
  if (p.exitCode !== 0) throw new Error(`git ${args.join(" ")}: ${p.stderr}`);
  return p.stdout.toString();
}

/** Repo-relative `site/...` paths from git, as site-relative paths. */
function siteFiles(out: string): string[] {
  return out
    .split("\n")
    .filter((f) => f.startsWith("site/"))
    .map((f) => f.slice("site/".length));
}

async function main(argv: string[]): Promise<number> {
  const root = process.cwd();
  const errors = await checkParity(root);

  const staged = argv.includes("--staged");
  const bi = argv.indexOf("--base");
  let changed: string[] | null = null;
  let message = process.env.TRANSLATIONS_UNCHANGED
    ? `Translations-Unchanged: ${process.env.TRANSLATIONS_UNCHANGED}`
    : "";
  if (staged) {
    changed = siteFiles(git(["diff", "--cached", "--name-only"]));
  } else if (bi >= 0) {
    const base = argv[bi + 1];
    if (base && !/^0+$/.test(base)) {
      changed = siteFiles(git(["diff", "--name-only", `${base}...HEAD`]));
      message += `\n${process.env.PR_BODY ?? ""}\n${git(["log", `${base}..HEAD`, "--format=%B"])}`;
    }
  }
  if (changed) errors.push(...checkChangeSync(root, changed, message));

  for (const e of errors) console.error(`translations: ${e}`);
  if (errors.length) {
    console.error(
      "Update every locale with its English change, or add a " +
        "`Translations-Unchanged: <reason>` line to the commit message or PR body.",
    );
    return 1;
  }
  console.log("translations: ok");
  return 0;
}

if (import.meta.main) process.exit(await main(process.argv.slice(2)));
