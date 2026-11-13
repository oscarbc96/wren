import { describe, expect, test } from "bun:test";

describe("built site", () => {
  test("home builds", async () => {
    const html = await Bun.file("dist/index.html").text();
    expect(html).toContain('<html lang="en"');
  });

  test("docs pages built", async () => {
    for (const page of [
      "docs/index.html",
      "docs/roadmap/index.html",
      "docs/build/index.html",
      "docs/contributing/index.html",
    ]) {
      expect(await Bun.file(`dist/${page}`).exists()).toBe(true);
    }
  });

  test("spanish home builds", async () => {
    const html = await Bun.file("dist/es/index.html").text();
    expect(html).toContain('<html lang="es"');
  });
});
