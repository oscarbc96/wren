import { describe, expect, test } from "bun:test";
import {
  checkChangeSync,
  checkParity,
  escapeReason,
  flattenKeys,
} from "../scripts/check-translations";

const fx = (name: string) => `tests/fixtures/translations/${name}`;
const pass = fx("pass");

describe("parity", () => {
  test("passes on a consistent tree", async () => {
    expect(await checkParity(pass)).toEqual([]);
  });
  test("passes on the real site", async () => {
    expect(await checkParity(".")).toEqual([]);
  });
  test("missing locale file", async () => {
    const e = await checkParity(fx("missing-locale-file"));
    expect(e).toEqual([
      expect.stringContaining("intro.es.mdx: missing translation"),
    ]);
  });
  test("orphan translation", async () => {
    const e = await checkParity(fx("orphan"));
    expect(e).toEqual([expect.stringContaining("extra.es.mdx: orphan")]);
  });
  test("meta mismatch", async () => {
    const e = await checkParity(fx("meta-mismatch"));
    expect(e).toEqual([
      expect.stringContaining('meta.es.json: missing page "intro"'),
    ]);
  });
  test("i18n key missing", async () => {
    const e = await checkParity(fx("i18n-key-missing"));
    expect(e).toEqual([
      expect.stringContaining('es.ts: missing key "nav.docs"'),
    ]);
  });
});

describe("change sync", () => {
  const en = "content/docs/intro.mdx";
  const es = "content/docs/intro.es.mdx";
  test("English without es fails", () => {
    expect(checkChangeSync(pass, [en])).toEqual([
      `${en} changed but ${es} did not`,
    ]);
  });
  test("English with es passes", () => {
    expect(checkChangeSync(pass, [en, es])).toEqual([]);
  });
  test("translation only passes", () => {
    expect(checkChangeSync(pass, [es])).toEqual([]);
  });
  test("meta and i18n tables need every locale", () => {
    expect(checkChangeSync(pass, ["content/docs/meta.json"])).toHaveLength(1);
    expect(checkChangeSync(pass, ["src/i18n/en.ts"])).toHaveLength(1);
    expect(checkChangeSync(pass, ["src/i18n/en.ts", "src/i18n/es.ts"])).toEqual(
      [],
    );
  });
  test("escape hatch with a reason skips the rule", () => {
    const msg = "fix typo\n\nTranslations-Unchanged: typo fix only";
    expect(checkChangeSync(pass, [en], msg)).toEqual([]);
  });
  test("escape hatch without a reason does not", () => {
    for (const msg of [
      "Translations-Unchanged:",
      "Translations-Unchanged:   \n",
    ]) {
      expect(checkChangeSync(pass, [en], msg)).toHaveLength(1);
    }
  });
});

test("escapeReason", () => {
  expect(escapeReason("a\nTranslations-Unchanged: why\nb")).toBe("why");
  expect(escapeReason("nothing")).toBeNull();
});

test("flattenKeys walks objects and arrays", () => {
  expect(flattenKeys({ a: { b: "x" }, c: [{ d: "y" }] }).sort()).toEqual([
    "a.b",
    "c.0.d",
  ]);
});
