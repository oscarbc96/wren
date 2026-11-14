import { describe, expect, test } from "bun:test";
import {
  fromCommits,
  groupSubjects,
  parseReleasePr,
  parseSubject,
  parseVersion,
} from "../src/lib/unreleased";

const repo = "https://github.com/example/wren-fixture";

describe("parseSubject", () => {
  test("parses type, scope, breaking marker", () => {
    expect(parseSubject("feat(app)!: new thing (#12)")).toEqual({
      type: "feat",
      scope: "app",
      breaking: true,
      description: "new thing (#12)",
    });
    expect(parseSubject("fix: plain")?.scope).toBeUndefined();
  });
  test("rejects non conventional subjects", () => {
    expect(parseSubject("Merge branch main")).toBeUndefined();
    expect(parseSubject("")).toBeUndefined();
  });
});

describe("groupSubjects", () => {
  test("groups like release-please and hides chores", () => {
    const md = groupSubjects(
      [
        "chore: bump",
        "fix(ui): crash (#2)",
        "feat(app): shiny (#1)",
        "ci: pipeline",
        "perf: faster",
        "refactor: tidy",
        "docs: words",
        "not conventional",
      ],
      repo,
    );
    expect(md).toBe(
      [
        "### Features\n\n* **app:** shiny ([#1](" + repo + "/pull/1))",
        "### Bug Fixes\n\n* **ui:** crash ([#2](" + repo + "/pull/2))",
        "### Performance Improvements\n\n* faster",
      ].join("\n\n"),
    );
  });
  test("breaking changes of hidden types are kept, first", () => {
    const md = groupSubjects(["feat: a", "refactor!: b"], repo);
    expect(md.startsWith("### Breaking Changes\n\n* b")).toBe(true);
    expect(md).toContain("### Features");
  });
  test("empty when nothing qualifies", () => {
    expect(groupSubjects(["chore: x"], repo)).toBe("");
    expect(fromCommits(["chore: x"], repo)).toBeUndefined();
    expect(fromCommits(["fix: y"], repo)?.source).toBe("commits");
  });
});

describe("parseReleasePr", () => {
  const body = `:robot: I have created a release *beep* *boop*
---


## [0.2.0](https://example.com/compare/v0.1.0...v0.2.0) (2031-02-01)


### Features

* thing ([#1](https://example.com/1))

---
This PR was generated with [Release Please](https://example.com). See [documentation](https://example.com).`;
  test("extracts version and notes", () => {
    const parsed = parseReleasePr({
      title: "chore(main): release 0.2.0",
      body,
      html_url: "https://example.com/pr/9",
    });
    expect(parsed?.version).toBe("0.2.0");
    expect(parsed?.url).toBe("https://example.com/pr/9");
    expect(parsed?.notes).toBe(
      "### Features\n\n* thing ([#1](https://example.com/1))",
    );
  });
  test("falls back to the heading version and tolerates empty input", () => {
    expect(parseReleasePr({ title: "release", body })?.version).toBe("0.2.0");
    expect(parseReleasePr({})).toBeUndefined();
  });
});

test("parseVersion", () => {
  expect(parseVersion("release 1.2.3-rc.1")).toBe("1.2.3-rc.1");
  expect(parseVersion("nothing")).toBeUndefined();
});
