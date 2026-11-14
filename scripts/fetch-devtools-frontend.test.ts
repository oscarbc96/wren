import { describe, expect, test } from "bun:test";
import {
  extractImports,
  extractNewUrlAssets,
  verifyLock,
} from "./fetch-devtools-frontend";

describe("fetch-devtools-frontend", () => {
  test("extracts static and dynamic imports", () => {
    const source = `
      import './a.js';
      import * as b from "./b.js";
      export {x} from '../c.js';
      export * from "/root/d.js";
      const e = import("./e.js");
      const f = import('../f.js');
    `;
    const specs = extractImports(source);
    for (const s of ["./a.js", "./b.js", "../c.js", "/root/d.js", "./e.js", "../f.js"]) {
      expect(specs).toContain(s);
    }
  });

  test("resolves new URL assets", () => {
    const source = `
      const a = new URL('./img/icon.png', import.meta.url);
      const b = new URL("../wasm/mod.wasm", import.meta.url);
      const c = new URL("data.json", import.meta.url);
    `;
    const assets = extractNewUrlAssets(source);
    expect(assets).toContain("./img/icon.png");
    expect(assets).toContain("../wasm/mod.wasm");
    expect(assets).toContain("data.json");
  });

  test("verify fails on hash mismatch", () => {
    const files = { "a.js": "aaa", "b.js": "bbb" };
    const lock = { "a.js": "aaa", "b.js": "ccc" };
    const errors = verifyLock(files, lock);
    expect(errors.length).toBeGreaterThan(0);
  });
});
