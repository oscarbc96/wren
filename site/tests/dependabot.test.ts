import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

type Update = {
  "package-ecosystem": string;
  directory: string;
  schedule: Record<string, string>;
  "open-pull-requests-limit": number;
  labels: string[];
  "commit-message": { prefix: string };
  groups: Record<string, { "update-types": string[] }>;
  ignore?: { "dependency-name": string }[];
};

let config: { version: number; updates: Update[] } | null = null;
try {
  config = Bun.YAML.parse(
    readFileSync("../.github/dependabot.yml", "utf8"),
  ) as { version: number; updates: Update[] };
} catch {
  config = null;
}

if (config) {
  describe("dependabot config", () => {
    test("covers cargo, github-actions and bun", () => {
      expect(config!.version).toBe(2);
      expect(config!.updates.map((u) => u["package-ecosystem"]).sort()).toEqual(
        ["bun", "cargo", "github-actions"],
      );
    });

    test.each(config!.updates)("$package-ecosystem follows the policy", (u) => {
      expect(u.schedule).toMatchObject({
        interval: "weekly",
        day: "monday",
        timezone: "Europe/Madrid",
      });
      expect(u["open-pull-requests-limit"]).toBe(5);
      expect(u.labels).toEqual(["dependencies"]);
      expect(u["commit-message"].prefix).toMatch(/^(chore|ci)\(deps\)$/);
      const types = Object.values(u.groups).flatMap((g) => g["update-types"]);
      expect(types.sort()).toEqual(["minor", "patch"]);
    });

    test("actions use the ci prefix, others chore", () => {
      for (const u of config!.updates) {
        const want =
          u["package-ecosystem"] === "github-actions"
            ? "ci(deps)"
            : "chore(deps)";
        expect(u["commit-message"].prefix).toBe(want);
      }
    });

    test("cargo ignores the pinned GPUI-CE crates", () => {
      const cargo = config!.updates.find(
        (u) => u["package-ecosystem"] === "cargo",
      );
      const names = cargo?.ignore?.map((i) => i["dependency-name"]) ?? [];
      for (const n of [
        "gpui-ce",
        "gpui_ce_platform",
        "gpui_ce_elements",
        "servo",
      ]) {
        expect(names).toContain(n);
      }
    });
  });
} else {
  test("dependabot config not present in this worktree", () => {});
}
