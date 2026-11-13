/**
 * Generates the static brand icons from the themeable master
 * `assets/brand/wren-icon.svg`.
 *
 * Satori, sharp and resvg do not resolve CSS `var()`, so every consumer that
 * rasterises needs literal colours:
 *
 * - `assets/brand/wren-icon-dark.svg` and `wren-icon-light.svg`: all colours
 *   are attributes, no `<style>` block.
 * - `site/public/favicon.svg`: still themeable, but switches through
 *   `@media (prefers-color-scheme: light)` instead of `data-theme`, and drops
 *   the blur halo to stay small.
 *
 *   bun scripts/brand.ts            write the files
 *   bun scripts/brand.ts --check    exit 1 if a generated file is stale
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

export type Theme = "dark" | "light";

const root = join(import.meta.dir, "..");
export const SOURCE = "assets/brand/wren-icon.svg";

/** Parses the `--wren-icon-*` custom properties of one `<style>` rule. */
export function themeVars(svg: string, theme: Theme): Record<string, string> {
  const selector = theme === "dark" ? "svg" : 'svg[data-theme="light"]';
  const start = svg.indexOf(`${selector} {`);
  if (start < 0) throw new Error(`no ${selector} rule in ${SOURCE}`);
  const block = svg.slice(start, svg.indexOf("}", start));
  const vars: Record<string, string> = {};
  for (const [, name, value] of block.matchAll(
    /--wren-icon-([\w-]+):\s*([^;]+);/g,
  ))
    vars[name] = value.trim();
  return vars;
}

const STYLE = /\s*<style>[\s\S]*?<\/style>\n?/;
const COMMENT = /\s*<!--\s*\n\s*Themeable[\s\S]*?-->\n?/;
const HALO = /^\s*<use [^>]*class="halo"[^>]*\/>\n/m;
const FILTER = /\s*<filter id="blur"[\s\S]*?<\/filter>\n?/;
// The wing overlay only matters when animated; drop it so static icons render exactly
// the master silhouette (no doubled antialiasing on the wing edge).
const WING = / {8}<!-- Wing:[\s\S]*?\n {8}<\/g>\n/;

function resolveClasses(svg: string, vars: Record<string, string>): string {
  return svg.replace(/<stop ([^>]*?) class="([\w-]+)"\/>/g, (_m, pre, cls) => {
    const value = vars[cls];
    if (!value) throw new Error(`unknown stop class ${cls}`);
    return `<stop ${pre} stop-color="${value}"/>`;
  });
}

/** Static variant: every colour literal, geometry untouched. */
export function staticVariant(source: string, theme: Theme): string {
  const vars = themeVars(source, theme);
  let svg = source.replace(COMMENT, "\n").replace(STYLE, "\n").replace(WING, "");
  svg = resolveClasses(svg, vars);
  const opacity = vars["glow-opacity"];
  if (Number(opacity) === 0) {
    svg = svg.replace(HALO, "").replace(FILTER, "\n");
  } else {
    svg = svg.replace(/(<use [^>]*?) class="halo"/, `$1 opacity="${opacity}"`);
  }
  return svg.replace(/\n{3,}/g, "\n\n");
}

/** Favicon: themeable through the OS colour scheme, no blur halo. */
export function faviconVariant(source: string): string {
  const dark = themeVars(source, "dark");
  const light = themeVars(source, "light");
  const decl = (v: Record<string, string>) =>
    Object.entries(v)
      .filter(([k]) => k !== "glow" && k !== "glow-opacity")
      .map(([k, val]) => `--wren-icon-${k}: ${val};`)
      .join(" ");
  const style = `<style>
    svg { ${decl(dark)} }
    @media (prefers-color-scheme: light) { svg { ${decl(light)} } }
    .bg-top { stop-color: var(--wren-icon-bg-top); }
    .bg-bottom { stop-color: var(--wren-icon-bg-bottom); }
    .teal-deep { stop-color: var(--wren-icon-teal-deep); }
    .teal { stop-color: var(--wren-icon-teal); }
    .coral { stop-color: var(--wren-icon-coral); }
    .coral-light { stop-color: var(--wren-icon-coral-light); }
  </style>`;
  return source
    .replace(COMMENT, "\n")
    .replace(STYLE, `\n  ${style}\n`)
    .replace(WING, "")
    .replace(HALO, "")
    .replace(FILTER, "\n")
    .replace(/\n{3,}/g, "\n\n");
}

/** Mark bounds (stroke included) plus room for the glow blur. */
export const MARK_VIEWBOX = "287 418 1363 1092";

/** Hero landing: flight and wing beats share this one duration. */
export const LANDING_SECONDS = 1.8;

/** Departure (scroll away) duration. */
export const DEPART_SECONDS = 1.4;

/** Take-off: a short lift, then away to the right edge of the viewport. */
const DEPART_KEYFRAMES = `
    0% { translate: 0 0; rotate: 0deg; animation-timing-function: ease-in; }
    15% { translate: 0 -16px; rotate: -8deg; animation-timing-function: ease-in-out; }
    35% { translate: 12vw -90px; rotate: -10deg; animation-timing-function: ease-in-out; }
    60% { translate: 42vw -60px; rotate: 4deg; animation-timing-function: ease-in-out; }
    80% { translate: 80vw -130px; rotate: -4deg; animation-timing-function: ease-in; }
    100% { translate: 120vw -80px; rotate: 0deg; }`;

/**
 * Scroll trigger: flight is chosen by [data-away]. A flight in progress always
 * finishes before the next one starts, so quick scrolling never jumps.
 */
const HERO_SCRIPT = `(function(){var b=document.querySelector(".wren-bird");if(!b||matchMedia("(prefers-reduced-motion: reduce)").matches)return;var busy=true,away=false,t;function want(){return scrollY>40}function run(){busy=true;clearTimeout(t);t=setTimeout(done,2400)}function done(){busy=false;sync()}function sync(){if(busy||want()===away)return;away=want();run();if(away)b.setAttribute("data-away","");else b.removeAttribute("data-away")}b.addEventListener("animationend",function(e){if(e.target===b&&e.animationName===(away?"wren-depart":"wren-fly"))done()});addEventListener("scroll",sync,{passive:true});if(want()){busy=false;sync()}else run()})()`;

/**
 * Shoulder pivot of the wing (source units 1055, 720) as a share of the wing
 * group's own bounding box (the bird silhouette, x 692-1272, y 527-913), so the
 * pivot does not depend on how a browser resolves the viewBox origin.
 */
const WING_ORIGIN = "62.6% 50%";

/**
 * Wing beats: steady beats in flight (0-60%), bigger braking flaps over the
 * last ~0.5s before touchdown (60-88%), then the wing folds into the static
 * pose with a small settle. Percentages follow the page's `wren-fly`.
 */
function wingKeyframes(mode: "land" | "away" = "land"): string {
  const pivot = (rot: number, sy: number) => `rotate(${rot}deg) scaleY(${sy})`;
  const stops: string[] = [];
  const phase = (
    from: number,
    to: number,
    beats: number,
    up: [number, number],
    down: [number, number],
  ) => {
    const steps = beats * 2;
    for (let i = 0; i <= steps; i++) {
      if (stops.length && i === 0) continue;
      const at = (from + ((to - from) * i) / steps).toFixed(2).replace(/\.?0+$/, "");
      const [rot, sy] = i % 2 === 0 ? up : down;
      stops.push(`${at || "0"}% { transform: ${pivot(rot, sy)}; }`);
    }
  };
  if (mode === "away") {
    // Take-off: big beats first, then steady cruise until the bird is gone.
    phase(0, 30, 3, [40, 0.68], [-22, 1.1]);
    phase(30, 100, 6, [24, 0.8], [-14, 1.08]);
    return stops.join("\n      ");
  }
  phase(0, 60, 5, [24, 0.8], [-14, 1.08]);
  phase(60, 88, 3, [40, 0.68], [-22, 1.1]);
  stops.push(`94% { transform: ${pivot(8, 0.95)}; }`);
  stops.push(`100% { transform: none; }`);
  return stops.join("\n      ");
}

/** Flight path: enters from the far left of the viewport, arcs, brakes, settles. */
const FLY_KEYFRAMES = `
    0% { translate: -100vw -60px; rotate: 0deg; animation-timing-function: ease-in-out; }
    20% { translate: -76vw -120px; rotate: -6deg; animation-timing-function: ease-in-out; }
    36% { translate: -52vw -70px; rotate: 4deg; animation-timing-function: ease-in-out; }
    52% { translate: -30vw -118px; rotate: -6deg; animation-timing-function: ease-in-out; }
    66% { translate: -12vw -62px; rotate: -8deg; animation-timing-function: ease-out; }
    80% { translate: -2vw -18px; rotate: -14deg; animation-timing-function: ease-out; }
    90% { translate: 0 7px; rotate: -3deg; animation-timing-function: ease-in-out; }
    96% { translate: 0 -3px; rotate: 1deg; animation-timing-function: ease-out; }
    100% { translate: 0 0; rotate: 0deg; }`;

const BIRD_GROUP = /^ {6}<g id="bird">[\s\S]*?\n {6}<\/g>\n/m;

/** The mark's children (ribbon, W, bird, arrowhead) without the wrapper. */
function markInner(source: string): string {
  const start = source.indexOf('<g id="mark">');
  const end = source.indexOf("</defs>");
  return source
    .slice(start, end)
    .trim()
    .replace(/^<g id="mark">/, "")
    .replace(/<\/g>\s*$/, "");
}

function defsOf(source: string): string {
  return source
    .slice(source.indexOf("<defs>") + 6, source.indexOf('<g id="mark">'))
    .replace(/\s*<linearGradient id="bg"[\s\S]*?<\/linearGradient>\n?/, "\n");
}

function wrap(
  source: string,
  theme: Theme,
  inner: string,
  style: string,
  { viewBox = MARK_VIEWBOX, withGlow = true } = {},
): string {
  const vars = themeVars(source, theme);
  const glow = withGlow && Number(vars["glow-opacity"]) > 0;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="${viewBox}" role="img" aria-labelledby="wren-icon-title">
  <title id="wren-icon-title">Wren</title>${style ? `\n  <style>${style}</style>` : ""}
  <defs>${defsOf(source)}  </defs>
${glow ? `  <g opacity="${vars["glow-opacity"]}" filter="url(#blur)">${inner}</g>\n` : ""}  <g>${inner}</g>
</svg>
`;
  const resolved = resolveClasses(svg, vars);
  return (glow ? resolved : resolved.replace(FILTER, "\n")).replace(
    /\n{3,}/g,
    "\n\n",
  );
}

/** Hero mark without the bird: bare, cropped, literal colours, glow in dark. */
export function markVariant(source: string, theme: Theme): string {
  return wrap(source, theme, markInner(source).replace(BIRD_GROUP, ""), "");
}

/** Prefix every id (and its references) and swap stop classes for CSS variables. */
function prefixSvg(svg: string, p: string): string {
  let out = svg.replace(/ xlink:href="[^"]*"/g, "");
  for (const id of HERO_IDS)
    out = out
      .replaceAll(`id="${id}"`, `id="${p}-${id}"`)
      .replaceAll(`url(#${id})`, `url(#${p}-${id})`)
      .replaceAll(`href="#${id}"`, `href="#${p}-${id}"`);
  return out
    .replace(
      /<stop ([^>]*?) class="([\w-]+)"\/>/g,
      (_m, pre, cls) => `<stop ${pre} style="stop-color: var(--wren-icon-${cls})"/>`,
    )
    .replace(/ xmlns:xlink="[^"]*"/, "");
}

const HERO_IDS = ["ink", "arrow", "blur", "cuts", "bird-shape", "wing-clip"];

/**
 * Hero bird as an inline Astro component. Inline (not an <img>) so the wing
 * beats and the flight run on the page's one CSS timeline: same start, same
 * duration. Colours come from --wren-icon-* variables (light on :root, dark
 * on .dark); every id is prefixed so nothing collides with other SVGs.
 */
export function heroBirdComponent(source: string): string {
  const bird = markInner(source).match(BIRD_GROUP)?.[0];
  if (!bird) throw new Error('no <g id="bird"> in source');
  const prefix = (svg: string) => prefixSvg(svg, "hb");
  const body = bird.replace('id="wing"', 'class="wing"');
  const defs = defsOf(source).replace(FILTER, (m) => m);
  const svg = prefix(`<svg class="wren-bird" xmlns="http://www.w3.org/2000/svg" viewBox="${MARK_VIEWBOX}" aria-hidden="true" focusable="false">
  <defs>${defs}  </defs>
  <g style="opacity: var(--wren-icon-glow-opacity)" filter="url(#blur)">
${body}  </g>
  <g>
${body}  </g>
</svg>`);
  const vars = (theme: Theme) =>
    Object.entries(themeVars(source, theme))
      .filter(([k]) => !k.startsWith("bg-"))
      .map(([k, v]) => `--wren-icon-${k}: ${v};`)
      .join(" ");
  return `---
// Generated by scripts/brand.ts from assets/brand/wren-icon.svg. Do not edit;
// run \`mise run brand\`.
---
${svg}

<style is:global>
  :root { ${vars("light")} }
  .dark { ${vars("dark")} }
  .wren-bird {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
    pointer-events: none;
  }
  .wren-bird .wing {
    transform-box: fill-box;
    transform-origin: ${WING_ORIGIN};
  }
  @media (prefers-reduced-motion: no-preference) {
    .wren-bird { animation: wren-fly ${LANDING_SECONDS}s both; }
    .wren-bird .wing { animation: wren-flap ${LANDING_SECONDS}s ease-in-out both; }
    .wren-bird[data-away] { animation-name: wren-depart; animation-duration: ${DEPART_SECONDS}s; }
    .wren-bird[data-away] .wing { animation-name: wren-flap-away; animation-duration: ${DEPART_SECONDS}s; }
  }
  @keyframes wren-fly {${FLY_KEYFRAMES}
  }
  @keyframes wren-flap {
      ${wingKeyframes()}
  }
  @keyframes wren-depart {${DEPART_KEYFRAMES}
  }
  @keyframes wren-flap-away {
      ${wingKeyframes("away")}
  }
</style>

<script is:inline>${HERO_SCRIPT}</script>
`;
}

/** Bird bounds (legs included) with a little padding. */
export const BIRD_VIEWBOX = "674 518 608 478";

/** Top-bar bird: inline, tightly cropped, no glow; the wing is animatable. */
export function navBirdSvg(source: string): string {
  const bird = markInner(source).match(BIRD_GROUP)?.[0];
  if (!bird) throw new Error('no <g id="bird"> in source');
  const defs = defsOf(source)
    .replace(/\s*<linearGradient id="arrow"[\s\S]*?<\/linearGradient>\n?/, "\n")
    .replace(FILTER, "\n");
  return prefixSvg(
    `<svg class="wren-navbird" xmlns="http://www.w3.org/2000/svg" viewBox="${BIRD_VIEWBOX}" aria-hidden="true" focusable="false"><defs>${defs}</defs>${bird.replace('id="wing"', 'class="wing"')}</svg>`,
    "nb",
  ).replace(/\n\s*/g, " ").replace(/<!--.*?-->/g, "");
}

type CssRule = {
  sel: string;
  decls?: [string, string][];
  rules?: CssRule[];
};

/** Prints rules the way `bun fmt` does (4 spaces), so the file is formatter-stable. */
function printCss(rules: CssRule[], depth = 0): string {
  const pad = "    ".repeat(depth);
  return rules
    .map((r) => {
      const decls = (r.decls ?? [])
        .map(([p, v]) => `${pad}    ${p}: ${v};\n`)
        .join("");
      const nested = r.rules ? printCss(r.rules, depth + 1) : "";
      return `${pad}${r.sel} {\n${decls}${nested}${pad}}\n`;
    })
    .join("");
}

/**
 * Hover/focus flap for the top-bar bird: a few quick beats and a small hop,
 * then back to rest. The link (a) that directly contains .wren-nav-bird is the
 * trigger, in both the Astro header and the Fumadocs nav title.
 */
export function navBirdCss(source: string): string {
  const vars = (theme: Theme): [string, string][] =>
    Object.entries(themeVars(source, theme))
      .filter(([k]) => !k.startsWith("bg-"))
      .map(([k, v]) => [`--wren-icon-${k}`, v]);
  const beat = (rot: number, sy: number) => `rotate(${rot}deg) scaleY(${sy})`;
  const flap: CssRule[] = [0, 1, 2, 3, 4, 5, 6, 7].map((i) => ({
    sel: `${((i / 7) * 82).toFixed(1).replace(/\.0$/, "")}%`,
    decls: [["transform", i % 2 === 0 ? beat(32, 0.72) : beat(-18, 1.08)]],
  }));
  flap.push({ sel: "100%", decls: [["transform", "none"]] });
  const hop = (at: number, y: string): CssRule => ({
    sel: `${at}%`,
    decls: [["transform", `translateY(${y})`]],
  });
  const trigger = "a:has(> .wren-nav-bird):is(:hover, :focus-visible) .wren-navbird";
  return (
    "/* Generated by scripts/brand.ts from assets/brand/wren-icon.svg. Do not edit; run `mise run brand`. */\n" +
    printCss([
      { sel: ":root", decls: vars("light") },
      { sel: ".dark", decls: vars("dark") },
      {
        sel: ".wren-nav-bird",
        decls: [
          ["display", "inline-flex"],
          ["align-items", "center"],
        ],
      },
      {
        sel: ".wren-navbird",
        decls: [
          ["height", "1.75rem"],
          ["width", "auto"],
          ["overflow", "visible"],
        ],
      },
      {
        sel: ".wren-navbird .wing",
        decls: [
          ["transform-box", "fill-box"],
          ["transform-origin", WING_ORIGIN],
        ],
      },
      {
        sel: "@media (prefers-reduced-motion: no-preference)",
        rules: [
          { sel: trigger, decls: [["animation", "wren-nav-hop 0.6s ease-out"]] },
          { sel: `${trigger} .wing`, decls: [["animation", "wren-nav-flap 0.6s ease-in-out"]] },
        ],
      },
      {
        sel: "@keyframes wren-nav-hop",
        rules: [hop(0, "0"), hop(35, "-4px"), hop(70, "1px"), hop(100, "0")],
      },
      { sel: "@keyframes wren-nav-flap", rules: flap },
    ])
  );
}

export const outputs = (source: string): Record<string, string> => ({
  "site/public/brand/wren-mark-dark.svg": markVariant(source, "dark"),
  "site/public/brand/wren-mark-light.svg": markVariant(source, "light"),
  "site/src/components/HeroBird.astro": heroBirdComponent(source),
  "site/src/styles/nav-bird.css": navBirdCss(source),
  "site/src/lib/nav-bird.generated.ts": `// Generated by scripts/brand.ts from assets/brand/wren-icon.svg. Do not edit;
// run \`mise run brand\`.
export const navBirdSvg =
  '${navBirdSvg(source)}';
`,
  "assets/brand/wren-icon-dark.svg": staticVariant(source, "dark"),
  "assets/brand/wren-icon-light.svg": staticVariant(source, "light"),
  "site/public/favicon.svg": faviconVariant(source),
});

function main(): void {
  const source = readFileSync(join(root, SOURCE), "utf8");
  const check = process.argv.includes("--check");
  let stale = 0;
  for (const [file, content] of Object.entries(outputs(source))) {
    const path = join(root, file);
    const current = existsSync(path) ? readFileSync(path, "utf8") : undefined;
    if (current === content) continue;
    if (check) {
      console.error(`stale: ${file} (run: mise run brand)`);
      stale++;
    } else {
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, content);
      console.log(`wrote ${file}`);
    }
  }
  if (stale) process.exit(1);
}

if (import.meta.main) main();
