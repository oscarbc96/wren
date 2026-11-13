import { describe, expect, test } from "bun:test";
import { parsePendingRelease } from "../src/lib/pending";

const pr = (body: string, title = "chore(main): release 1.2.3") => ({
  number: 7,
  title,
  html_url: "https://github.com/o/r/pull/7",
  body,
});

describe("parsePendingRelease", () => {
  test("extracts the version, notes and PR link", () => {
    const entry = parsePendingRelease(
      pr(
        ":robot: I have created a release *beep* *boop*\n---\n\n## [1.2.3](https://x/compare/v1.2.2...v1.2.3) (2031-01-01)\n\n### Features\n\n* a thing\n\n---\nThis PR was generated with [Release Please](https://x).",
      ),
    );
    expect(entry?.version).toBe("1.2.3");
    expect(entry?.url).toBe("https://github.com/o/r/pull/7");
    expect(entry?.content).toBe("### Features\n\n* a thing");
  });

  test("handles an unlinked heading", () => {
    const entry = parsePendingRelease(
      pr(
        "---\n\n## 1.2.3 (2031-01-01)\n\n* x\n\n---\nThis PR was generated with Release Please.",
      ),
    );
    expect(entry?.version).toBe("1.2.3");
    expect(entry?.content).toBe("* x");
  });

  test("falls back to the title for the version", () => {
    const entry = parsePendingRelease(pr("just some notes"));
    expect(entry?.version).toBe("1.2.3");
    expect(entry?.content).toBe("just some notes");
  });

  test("returns undefined when no version can be found", () => {
    expect(parsePendingRelease(pr("notes", "something else"))).toBeUndefined();
    expect(parsePendingRelease(null)).toBeUndefined();
    expect(parsePendingRelease({ number: "x" })).toBeUndefined();
  });
});
