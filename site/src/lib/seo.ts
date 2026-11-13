import { defaultLocale, localeCodes, localePath } from "@/i18n";

/**
 * Locale-generic SEO helpers. Everything is derived from `src/i18n/config.json`,
 * so adding a locale needs no change here beyond an optional `ogLocales` entry.
 */

export const siteName = "Wren";
export const repoUrl = "https://github.com/oscarbc96/wren";

const origin = (import.meta.env.SITE ?? "https://oscarbc96.github.io").replace(
  /\/$/,
  "",
);

/** Open Graph locale (`language_TERRITORY`) per locale code. */
const ogLocales: Record<string, string> = {
  en: "en_US",
  es: "es_ES",
  ca: "ca_ES",
};

export function ogLocale(code: string): string {
  return ogLocales[code] ?? `${code}_${code.toUpperCase()}`;
}

/** Absolute URL of `path` (starting with `/`) in `locale`. */
export function absoluteUrl(locale: string, path = "/"): string {
  return `${origin}${localePath(locale, path)}`;
}

const base = import.meta.env.BASE_URL.replace(/\/$/, "");

/** Absolute URL of a file under the site base (not locale-prefixed). */
export function siteUrl(path: string): string {
  return `${origin}${base}${path}`;
}

export interface Alternate {
  hreflang: string;
  href: string;
}

/** hreflang alternates for the page at `path`: every locale plus x-default. */
export function alternatesFor(path: string): Alternate[] {
  return [
    ...localeCodes.map((code) => ({
      hreflang: code,
      href: absoluteUrl(code, path),
    })),
    { hreflang: "x-default", href: absoluteUrl(defaultLocale, path) },
  ];
}

/** Absolute URL of the generated Open Graph image for `key` (see `lib/pages.ts`). */
export function ogImageUrl(locale: string, key: string): string {
  return siteUrl(`/og/${locale}/${key}.png`);
}
