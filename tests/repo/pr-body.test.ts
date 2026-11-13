import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  checkPrBody,
  isHotPath,
  isUiPath,
  sections,
  stripComments,
} from "../../scripts/pr-body";

const root = join(import.meta.dir, "..", "..");
const template = readFileSync(
  join(root, ".github/pull_request_template.md"),
  "utf8",
);
const good = readFileSync(
  join(import.meta.dir, "fixtures/pr-body-good.md"),
  "utf8",
);
const title = "feat(components): pin messages in a channel";

const check = (body: string, changed: string[] = [], t = title) =>
  checkPrBody({ body, title: t, changed });

describe("parsing", () => {
  test("strips HTML comments, including multi-line ones", () => {
    expect(stripComments("a<!-- x\ny -->b")).toBe("ab");
  });
  test("splits level-two sections", () => {
    const s = sections("## One\nx\n## Two\n\ny\n");
    expect([...s.keys()]).toEqual(["One", "Two"]);
    expect(s.get("Two")?.trim()).toBe("y");
  });
});

describe("the template itself", () => {
  test("an unfilled template fails on every required section", () => {
    const e = check(template);
    for (const name of [
      "Summary",
      "Plan task or issue",
      "Type of change (pick one)",
      "User-visible change",
      "Screenshots and visual diff",
      "Performance",
    ]) {
      expect(e.some((m) => m.startsWith(`${name}:`))).toBe(true);
    }
  });
  test("a filled body passes", () => {
    expect(check(good)).toEqual([]);
  });
});

describe("sections", () => {
  test("missing section", () => {
    const body = good.replace(/## Performance[\s\S]*?(?=## Tests)/, "");
    expect(check(body)).toContainEqual(
      expect.stringContaining("Performance: missing section"),
    );
  });
  test("empty section (only comments)", () => {
    const body = good.replace(
      /## Summary\n[\s\S]*?(?=## Plan)/,
      "## Summary\n\n<!-- fill this -->\n\n",
    );
    expect(check(body)).toContainEqual("Summary: empty; fill it in");
  });
  test("bare N/A without a reason fails", () => {
    const body = good.replace(
      /## Plan task or issue\n[\s\S]*?(?=## Type)/,
      "## Plan task or issue\n\nN/A\n\n",
    );
    expect(check(body)).toContainEqual(
      expect.stringContaining('Plan task or issue: "N/A" needs a reason'),
    );
  });
  test("N/A with a reason passes", () => {
    const body = good.replace(
      /## Plan task or issue\n[\s\S]*?(?=## Type)/,
      "## Plan task or issue\n\nN/A: owner request, no plan task\n\n",
    );
    expect(check(body)).toEqual([]);
  });
});

describe("checkboxes", () => {
  test("an unticked box without N/A fails", () => {
    const body = good.replace(
      "- [x] `mise run build`",
      "- [ ] `mise run build`",
    );
    expect(check(body)).toContainEqual(
      expect.stringContaining(
        'Checks run locally: unticked box needs "N/A: <reason>"',
      ),
    );
  });
  test("an unticked box with N/A and a reason passes", () => {
    const body = good.replace(
      "- [x] `mise run build`",
      "- [ ] `mise run build` N/A: docs only",
    );
    expect(check(body)).toEqual([]);
  });
  test("an unticked box with an empty N/A fails", () => {
    const body = good.replace(
      "- [x] `mise run build`",
      "- [ ] `mise run build` N/A:",
    );
    expect(check(body).length).toBe(1);
  });
  test("upper-case X counts as ticked", () => {
    const body = good.replace(
      "- [x] `mise run build`",
      "- [X] `mise run build`",
    );
    expect(check(body)).toEqual([]);
  });
});

describe("type of change", () => {
  test("needs exactly one ticked box", () => {
    const body = good.replace("- [x] `feat`", "- [ ] `feat`");
    expect(check(body)).toContainEqual(
      "Type of change (pick one): tick exactly one box (found 0)",
    );
  });
  test("must match the PR title type", () => {
    expect(check(good, [], "fix(components): pin messages")).toContainEqual(
      expect.stringContaining("ticked `feat` but the title type is `fix`"),
    );
  });
  test("title must be a Conventional Commit", () => {
    expect(check(good, [], "Pin messages")).toContainEqual(
      expect.stringContaining("Title: not a Conventional Commit"),
    );
  });
  test("breaking title needs a filled breaking section", () => {
    const body = good.replace(
      /## Breaking changes and migrations\n[\s\S]*?(?=## Release note)/,
      "## Breaking changes and migrations\n\nN/A: nothing breaks\n\n",
    );
    expect(check(body, [], "feat(components)!: pin messages")).toContainEqual(
      expect.stringContaining(
        "Breaking changes and migrations: the title has `!`",
      ),
    );
  });
});

describe("UI evidence", () => {
  const noShots = good.replace(
    /## Screenshots and visual diff\n[\s\S]*?(?=## Performance)/,
    "## Screenshots and visual diff\n\nN/A: no UI change\n\n",
  );
  test("classifies UI paths", () => {
    expect(isUiPath("crates/components/src/sidebar.rs")).toBe(true);
    expect(isUiPath("crates/ui_testing/baselines/macos/sidebar-dark.png")).toBe(
      true,
    );
    expect(isUiPath("assets/themes/wren-dark.json")).toBe(true);
    expect(isUiPath("site/src/components/Hero.tsx")).toBe(true);
    expect(isUiPath("crates/store_sqlite/src/lib.rs")).toBe(false);
    expect(isUiPath("site/content/docs/index.mdx")).toBe(false);
  });
  test("UI change without screenshots or a visual report fails", () => {
    expect(check(noShots, ["crates/components/src/sidebar.rs"])).toContainEqual(
      expect.stringContaining("Screenshots and visual diff: UI files changed"),
    );
  });
  test("no UI change: N/A is fine", () => {
    expect(check(noShots, ["docs/specs/2026-11-12-design.md"])).toEqual([]);
  });
  test("a visual report link alone is enough", () => {
    const body = noShots.replace(
      "N/A: no UI change",
      "Visual report: https://github.com/o/r/actions/runs/1/artifacts/2",
    );
    expect(check(body, ["crates/components/src/sidebar.rs"])).toEqual([]);
  });
  test("UI-Unchanged with a reason skips the rule", () => {
    const body = `${noShots}\nUI-Unchanged: runtime wiring only, nothing rendered\n`;
    expect(check(body, ["crates/wren/src/runtime.rs"])).toEqual([]);
  });
  test("UI-Unchanged inside a comment does not count", () => {
    const body = `${noShots}\n<!-- UI-Unchanged: hidden -->\n`;
    expect(check(body, ["crates/components/src/sidebar.rs"]).length).toBe(1);
  });
});

describe("performance evidence", () => {
  const noNumbers = good.replace(
    /## Performance\n[\s\S]*?(?=## Tests)/,
    "## Performance\n\nN/A: not a hot path\n\n",
  );
  test("classifies hot paths", () => {
    expect(isHotPath("crates/domain/src/reducer.rs")).toBe(true);
    expect(isHotPath("crates/wren/tests/perf_budget.toml")).toBe(true);
    expect(isHotPath("crates/providers/foo/realtime/src/conn.rs")).toBe(
      true,
    );
    expect(isHotPath("crates/recon/src/main.rs")).toBe(false);
    expect(isHotPath("crates/domain/tests/reducer.rs")).toBe(false);
  });
  test("hot path without numbers fails", () => {
    expect(check(noNumbers, ["crates/domain/src/reducer.rs"])).toContainEqual(
      expect.stringContaining("Performance: hot-path files changed"),
    );
  });
  test("hot path with Perf-Unaffected passes", () => {
    const body = noNumbers.replace(
      "N/A: not a hot path",
      "Perf-Unaffected: renames a private field",
    );
    expect(check(body, ["crates/domain/src/reducer.rs"])).toEqual([]);
  });
  test("no hot path: N/A is fine", () => {
    expect(check(noNumbers, ["README.md"])).toEqual([]);
  });
});

describe("security section", () => {
  test("N/A is not accepted there", () => {
    const body = good.replace(
      "- [x] No employer named",
      "- [ ] No employer named N/A: not relevant",
    );
    expect(check(body)).toContainEqual(
      expect.stringContaining(
        "Security, privacy and confidentiality: every box must be ticked",
      ),
    );
  });
});

describe("workflow", () => {
  const wf = Bun.YAML.parse(
    readFileSync(join(root, ".github/workflows/pr-template.yml"), "utf8"),
  ) as any;
  const steps: any[] = wf.jobs.body.steps;
  test("reads body and title through env, never inline in run", () => {
    const run = steps.find((s) => s.run?.includes("pr-body.ts"));
    expect(run.env.PR_BODY).toBe("${{ github.event.pull_request.body }}");
    expect(run.env.PR_TITLE).toBe("${{ github.event.pull_request.title }}");
    for (const s of steps) expect(s.run ?? "").not.toContain("${{");
  });
  test("least privilege and SHA-pinned actions", () => {
    expect(wf.permissions).toEqual({});
    expect(wf.jobs.body.permissions).toEqual({ contents: "read" });
    for (const s of steps.filter((s) => s.uses)) {
      expect(s.uses).toMatch(/@[0-9a-f]{40}$/);
    }
  });
  test("re-runs when the body is edited", () => {
    expect(wf.on.pull_request.types).toContain("edited");
  });
});
