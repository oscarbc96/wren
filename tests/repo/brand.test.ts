import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  BIRD_VIEWBOX,
  navBirdCss,
  navBirdSvg,
  heroBirdComponent,
  faviconVariant,
  markVariant,
  MARK_VIEWBOX,
  outputs,
  SOURCE,
  staticVariant,
  themeVars,
} from "../../scripts/brand";

const root = join(import.meta.dir, "..", "..");
const source = readFileSync(join(root, SOURCE), "utf8");

describe("generated files", () => {
  for (const [file, content] of Object.entries(outputs(source))) {
    test(`${file} is up to date`, () => {
      expect(readFileSync(join(root, file), "utf8")).toBe(content);
    });
  }
});

describe("static variants", () => {
  const dark = staticVariant(source, "dark");
  const light = staticVariant(source, "light");

  test("contain no var() or style block", () => {
    for (const svg of [dark, light]) {
      expect(svg).not.toContain("var(");
      expect(svg).not.toContain("<style");
      expect(svg).not.toContain('class="');
    }
  });
  test("dark has the dark palette and a literal halo opacity", () => {
    for (const c of ["#131a2c", "#10172a", "#20e8dd", "#4ab9c0", "#ff7672"])
      expect(dark).toContain(`stop-color="${c}"`);
    expect(dark).toContain('opacity="0.55"');
    expect(dark).toContain('filter="url(#blur)"');
  });
  test("light has the light palette and no halo", () => {
    for (const c of ["#ffffff", "#f5f8f9", "#1592a5", "#4cc3c5", "#ff7671"])
      expect(light).toContain(`stop-color="${c}"`);
    expect(light).not.toContain("#10172a");
    expect(light).not.toContain("blur");
  });
  test("geometry is identical to the source", () => {
    const paths = (s: string) => s.match(/ d="[^"]+"/g);
    expect(paths(dark)).toEqual(paths(source));
  });
});

describe("hero mark and bird", () => {
  const markD = markVariant(source, "dark");
  const markL = markVariant(source, "light");

  test("the mark has the shared viewBox, no tile, no var() and literal colours", () => {
    for (const svg of [markD, markL]) {
      expect(svg).not.toContain("var(");
      expect(svg).not.toContain('rx="330"');
      expect(svg).not.toContain('id="bg"');
      expect(svg).toContain(`viewBox="${MARK_VIEWBOX}"`);
    }
    expect(markD).toContain('stop-color="#20e8dd"');
    expect(markL).toContain('stop-color="#1592a5"');
  });
  test("the mark has no bird and does not animate", () => {
    for (const svg of [markD, markL]) {
      expect(svg).not.toContain('id="bird"');
      expect(svg).not.toContain('class="wing"');
      expect(svg).not.toContain("@keyframes");
      expect(svg).toContain("W, ending in the arrow shaft");
    }
  });
  test("the hero bird is one inline component on the page's timeline", () => {
    const c = heroBirdComponent(source);
    // Inline, not an <img>: flight and wing beats start together on one clock.
    expect(c).toContain('<svg class="wren-bird"');
    expect(c).toContain("@keyframes wren-fly");
    expect(c).toContain("@keyframes wren-flap");
    expect(c).toContain("animation: wren-fly 1.8s both");
    expect(c).toContain("animation: wren-flap 1.8s ease-in-out both");
    expect(c).toContain("prefers-reduced-motion: no-preference");
    // Both modes from CSS variables, one markup.
    expect(c).toContain(":root { --wren-icon-teal-deep: #1592a5;");
    expect(c).toContain(".dark { --wren-icon-teal-deep: #20e8dd;");
    expect(c).toContain("var(--wren-icon-glow-opacity)");
    // Only the bird, ids prefixed, glow copy and main copy both animate.
    expect(c).not.toContain("W, ending in the arrow shaft");
    expect(c).not.toContain('id="ink"');
    expect(c).toContain('id="hb-ink"');
    expect(c).not.toMatch(/<img|xlink/);
    expect(c.match(/class="wing"/g)?.length).toBe(2);
  });
  test("the wing flaps through flight and braking, then settles", () => {
    const c = heroBirdComponent(source);
    const flap = c.slice(c.indexOf("@keyframes wren-flap"));
    expect(flap.match(/rotate\(24deg\)/g)?.length).toBeGreaterThanOrEqual(6);
    expect(flap).toContain("rotate(40deg)");
    expect(flap).toMatch(/100% \{ transform: none; \}/);
  });
  test("dark keeps a glow on the mark, light has none", () => {
    expect(markD).toContain('filter="url(#blur)"');
    expect(markL).not.toContain("blur");
  });
  test("the wing is its own group inside the bird in the source", () => {
    expect(source).toMatch(/<g id="bird">[\s\S]*<g id="wing">/);
  });
  test("the nav bird is inline, tightly cropped, and flaps on hover or focus", () => {
    const svg = navBirdSvg(source);
    expect(svg).toContain(`viewBox="${BIRD_VIEWBOX}"`);
    expect(svg).toContain('class="wing"');
    expect(svg).toContain('id="nb-ink"');
    expect(svg).not.toMatch(/blur|<style|xlink|W, ending/);
    const css = navBirdCss(source);
    expect(css).toContain(":is(:hover, :focus-visible)");
    expect(css).toContain("@keyframes wren-nav-flap");
    expect(css).toContain("prefers-reduced-motion: no-preference");
    expect(css).toContain(".dark {\n    --wren-icon-teal-deep: #20e8dd;");
  });
  test("static icons drop the wing overlay", () => {
    expect(staticVariant(source, "dark")).not.toContain('id="wing"');
    expect(faviconVariant(source)).not.toContain('id="wing"');
  });
});

describe("favicon", () => {
  const fav = faviconVariant(source);
  test("switches by prefers-color-scheme, not data-theme", () => {
    expect(fav).toContain("prefers-color-scheme: light");
    expect(fav).not.toContain("data-theme");
    expect(fav).not.toContain("blur");
  });
  test("carries both palettes", () => {
    expect(fav).toContain("#10172a");
    expect(fav).toContain("#f5f8f9");
  });
});

test("themeVars parses both themes", () => {
  expect(themeVars(source, "dark")["glow-opacity"]).toBe("0.55");
  expect(themeVars(source, "light")["glow-opacity"]).toBe("0");
});
