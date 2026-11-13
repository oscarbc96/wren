/**
 * Checks a pull request body against `.github/pull_request_template.md`.
 *
 * - Every required `##` section is present and has text outside HTML comments.
 * - `N/A` always carries a reason: `N/A: <reason>`.
 * - Every unticked box ends with `N/A: <reason>`, except in "(pick one)"
 *   sections (exactly one ticked) and the security section (all ticked).
 * - The ticked type matches the Conventional Commit type of the title.
 * - UI files changed: screenshots or a visual-report link, unless the body
 *   has a `UI-Unchanged: <reason>` line.
 * - Hot-path files changed: numbers in Performance, unless the body has a
 *   `Perf-Unaffected: <reason>` line.
 *
 * The body and title come from the environment (`PR_BODY`, `PR_TITLE`) or
 * from `--body-file <file>` and `--title <title>`, never from the command
 * line of a workflow. Changed files come from `git diff <base>...HEAD`.
 *
 *   bun scripts/pr-body.ts [--body-file <f>] [--title <t>] [--base <sha>]
 */
import { readFileSync } from "node:fs";

export const REQUIRED = [
  "Summary",
  "Plan task or issue",
  "Type of change (pick one)",
  "User-visible change",
  "Screenshots and visual diff",
  "Performance",
  "Tests",
  "Docs and translations",
  "Checks run locally",
  "Security, privacy and confidentiality",
  "Breaking changes and migrations",
  "Release note",
];
const PICK_ONE = "Type of change (pick one)";
const ALL_TICKED = "Security, privacy and confidentiality";
const SHOTS = "Screenshots and visual diff";
const PERF = "Performance";
const BREAKING = "Breaking changes and migrations";

export const TITLE =
  /^(feat|fix|perf|docs|refactor|test|ci|build|chore|revert)(\([a-z0-9_./-]+\))?(!)?: \S.*$/;
const BOX = /^\s*[-*] \[( |x|X)\]\s*(.*)$/;
const NA_REASON = /\bN\/A:\s*\S/;
const BARE_NA = /^\s*N\/A\s*:?\s*$/;

const UI_PATHS = [
  /^crates\/components\/src\//,
  /^crates\/themes\//,
  /^crates\/wren\/src\//,
  /^crates\/ui_testing\/baselines\//,
  /^crates\/providers\/[^/]+\/[^/]+\/src\/(render|ui)/,
  /^assets\/(themes|icons)\//,
  /^site\/src\//,
  /^site\/public\//,
];
const HOT_PATHS = [
  /^crates\/(domain|engine|tabs|store|store_sqlite|omnibox|components|wren|calls|i18n)\/src\//,
  /^crates\/providers\/[^/]+\/(realtime|markup|provider)\/src\//,
  /^crates\/wren\/tests\/perf(\.rs|_budget\.toml)$/,
];

export const isUiPath = (p: string) => UI_PATHS.some((r) => r.test(p));
export const isHotPath = (p: string) => HOT_PATHS.some((r) => r.test(p));

export function stripComments(s: string): string {
  return s.replace(/<!--[\s\S]*?(-->|$)/g, "");
}

/** Level-two sections of a comment-free body, keyed by heading text. */
export function sections(body: string): Map<string, string> {
  const out = new Map<string, string>();
  let name: string | null = null;
  let buf: string[] = [];
  for (const line of body.split(/\r?\n/)) {
    const h = /^##\s+(.+?)\s*$/.exec(line);
    if (h) {
      if (name !== null) out.set(name, buf.join("\n"));
      name = h[1];
      buf = [];
    } else if (name !== null) {
      buf.push(line);
    }
  }
  if (name !== null) out.set(name, buf.join("\n"));
  return out;
}

const escape = (body: string, key: string) =>
  new RegExp(`^[ \\t]*${key}:[ \\t]*\\S`, "m").test(body);

export interface Input {
  body: string;
  title?: string;
  changed?: string[];
}

export function checkPrBody({ body, title, changed = [] }: Input): string[] {
  const errors: string[] = [];
  const clean = stripComments(body);
  const secs = sections(clean);
  let titleType: string | null = null;
  let breaking = false;

  if (title !== undefined) {
    const m = TITLE.exec(title.trim());
    if (!m) {
      errors.push(
        "Title: not a Conventional Commit `type(scope): summary` " +
          "(types: feat, fix, perf, docs, refactor, test, ci, build, chore, revert)",
      );
    } else {
      titleType = m[1];
      breaking = m[3] === "!";
    }
  }

  for (const name of REQUIRED) {
    const text = secs.get(name);
    if (text === undefined) {
      errors.push(
        `${name}: missing section; keep every heading of the template`,
      );
      continue;
    }
    const lines = text.split("\n").filter((l) => l.trim() !== "");
    if (lines.length === 0) {
      errors.push(`${name}: empty; fill it in`);
      continue;
    }
    if (lines.some((l) => BARE_NA.test(l))) {
      errors.push(`${name}: "N/A" needs a reason, as "N/A: <reason>"`);
    }
    const boxes = lines.map((l) => BOX.exec(l)).filter((b) => b !== null);
    const ticked = boxes.filter((b) => b[1] !== " ");
    if (name === PICK_ONE) {
      if (ticked.length !== 1) {
        errors.push(`${name}: tick exactly one box (found ${ticked.length})`);
      } else if (titleType) {
        const picked = /`([a-z]+)`/.exec(ticked[0][2])?.[1];
        if (picked !== titleType) {
          errors.push(
            `${name}: ticked \`${picked}\` but the title type is \`${titleType}\``,
          );
        }
      }
      continue;
    }
    for (const b of boxes) {
      if (b[1] !== " ") continue;
      const item = b[2].trim().slice(0, 60);
      if (name === ALL_TICKED) {
        errors.push(`${name}: every box must be ticked: "${item}"`);
      } else if (!NA_REASON.test(b[2])) {
        errors.push(`${name}: unticked box needs "N/A: <reason>": "${item}"`);
      }
    }
  }

  const naOnly = (t: string | undefined) =>
    t !== undefined &&
    /^\s*N\/A:/.test(t.trim()) &&
    t.trim().split("\n").length === 1;

  if (breaking && naOnly(secs.get(BREAKING))) {
    errors.push(
      `${BREAKING}: the title has \`!\`; describe what breaks and the migration`,
    );
  }

  const ui = changed.filter(isUiPath);
  if (ui.length && !escape(clean, "UI-Unchanged")) {
    const t = secs.get(SHOTS) ?? "";
    const image = /!\[[^\]]*\]\([^)]+\)|<img\s/i.test(t);
    const report = /https?:\/\/\S*\/actions\/runs\/\d+/.test(t);
    if (!image && !report) {
      errors.push(
        `${SHOTS}: UI files changed (${ui.slice(0, 3).join(", ")}` +
          `${ui.length > 3 ? ", ..." : ""}); add before/after screenshots or the ` +
          "`mise run test:visual` report link, or a `UI-Unchanged: <reason>` line",
      );
    }
  }

  const hot = changed.filter(isHotPath);
  if (hot.length && !escape(clean, "Perf-Unaffected")) {
    const t = secs.get(PERF) ?? "";
    if (naOnly(t) || !/\d/.test(t)) {
      errors.push(
        `${PERF}: hot-path files changed (${hot.slice(0, 3).join(", ")}` +
          `${hot.length > 3 ? ", ..." : ""}); give \`mise run test:perf\` numbers ` +
          "against the budgets, or a `Perf-Unaffected: <reason>` line",
      );
    }
  }

  return errors;
}

function git(args: string[]): string {
  const r = Bun.spawnSync(["git", ...args], { stdout: "pipe", stderr: "pipe" });
  if (r.exitCode !== 0)
    throw new Error(`git ${args.join(" ")}: ${r.stderr.toString()}`);
  return r.stdout.toString();
}

function arg(argv: string[], name: string): string | undefined {
  const i = argv.indexOf(name);
  return i >= 0 ? argv[i + 1] : undefined;
}

export function main(argv: string[]): number {
  const file = arg(argv, "--body-file");
  const body = file ? readFileSync(file, "utf8") : (process.env.PR_BODY ?? "");
  const title = arg(argv, "--title") ?? process.env.PR_TITLE;
  const base = arg(argv, "--base") ?? process.env.BASE_SHA;
  const changed =
    base && !/^0+$/.test(base)
      ? git(["diff", "--name-only", `${base}...HEAD`])
          .split("\n")
          .filter(Boolean)
      : [];
  const errors = checkPrBody({ body, title, changed });
  for (const e of errors) console.error(`pr-body: ${e}`);
  if (errors.length) {
    console.error(
      "Fill the PR body as `.github/pull_request_template.md` asks, then edit the PR; the check re-runs.",
    );
    return 1;
  }
  console.log("pr-body: ok");
  return 0;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
