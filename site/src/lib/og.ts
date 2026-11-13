import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import satori from "satori";
import sharp from "sharp";
// Static dark variant (literal colours; satori cannot resolve CSS var()).
// Regenerate with `mise run brand`.
import markSvg from "../../../assets/brand/wren-icon-dark.svg?raw";

/**
 * Build-time Open Graph images: satori lays out a card (own brand only, no
 * third-party marks) and sharp rasterises it to a 1200x630 PNG.
 */

const require = createRequire(import.meta.url);
const fontDir = require
  .resolve("@fontsource/inter/package.json")
  .replace(/package\.json$/, "files/");

async function loadFonts() {
  const files = [
    ["inter-latin-700-normal.woff", 700],
    ["inter-latin-ext-700-normal.woff", 700],
    ["inter-latin-400-normal.woff", 400],
    ["inter-latin-ext-400-normal.woff", 400],
  ] as const;
  return Promise.all(
    files.map(async ([file, weight]) => ({
      name: "Inter",
      data: await readFile(`${fontDir}${file}`),
      weight,
      style: "normal" as const,
    })),
  );
}

let fonts: ReturnType<typeof loadFonts> | undefined;

type Node = { type: string; props: Record<string, unknown> };

function el(
  type: string,
  style: Record<string, unknown>,
  children?: unknown,
  extra: Record<string, unknown> = {},
): Node {
  return { type, props: { style, children, ...extra } };
}

const mark = `data:image/svg+xml;utf8,${encodeURIComponent(markSvg)}`;

export interface OgCard {
  locale: string;
  title: string;
  description: string;
}

/** The card as a PNG buffer. */
export async function renderOgImage({
  locale,
  title,
  description,
}: OgCard): Promise<Buffer> {
  void locale;
  fonts ??= loadFonts();
  const headline = title.replace(/\s*\|\s*Wren$/, "");
  const tree = el(
    "div",
    {
      width: 1200,
      height: 630,
      display: "flex",
      flexDirection: "column",
      justifyContent: "space-between",
      boxSizing: "border-box",
      padding: 72,
      background: "#10172a",
      color: "#e8edf5",
      fontFamily: "Inter",
    },
    [
      el("div", { display: "flex", alignItems: "center" }, [
        el("img", { width: 80, height: 80, marginRight: 24 }, undefined, {
          src: mark,
          width: 80,
          height: 80,
        }),
        el(
          "div",
          {
            fontSize: 56,
            fontWeight: 700,
            letterSpacing: -2,
            color: "#ff7672",
          },
          "Wren",
        ),
      ]),
      el("div", { display: "flex", flexDirection: "column" }, [
        el(
          "div",
          {
            fontSize: headline.length > 60 ? 54 : 68,
            fontWeight: 700,
            lineHeight: 1.1,
            letterSpacing: -1.5,
          },
          headline,
        ),
        el(
          "div",
          {
            fontSize: 30,
            lineHeight: 1.4,
            marginTop: 28,
            color: "#9aa6bd",
            fontWeight: 400,
          },
          description,
        ),
      ]),
      el(
        "div",
        { display: "flex", fontSize: 22, color: "#9aa6bd", fontWeight: 400 },
        "Wren",
      ),
    ],
  );
  const svg = await satori(tree as never, {
    width: 1200,
    height: 630,
    fonts: await fonts,
  });
  return sharp(Buffer.from(svg))
    .png({ compressionLevel: 9, palette: true })
    .toBuffer();
}
