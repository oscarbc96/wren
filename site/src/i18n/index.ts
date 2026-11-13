import type { I18nConfig } from "fumadocs-core/i18n";
import config from "@/i18n/config.json";
import { en } from "@/i18n/en";
import { es } from "@/i18n/es";
import type { Ui } from "@/i18n/ui";

/**
 * Adding a locale: add it to `config.json`, add its UI strings to the `ui`
 * map below, and add `*.<code>.mdx` (and `meta.<code>.json`) docs files.
 */
const ui: Record<string, Ui> = { en, es };

export const defaultLocale = config.defaultLocale;
export const locales = config.locales;
export const localeCodes = locales.map((l) => l.code);
export const nonDefaultLocales = localeCodes.filter((c) => c !== defaultLocale);

export const i18n = {
  languages: localeCodes,
  defaultLanguage: defaultLocale,
  hideLocale: "default-locale",
  parser: "dot",
} satisfies I18nConfig;

const base = import.meta.env.BASE_URL.replace(/\/$/, "");

export function isLocale(code: string | undefined): code is string {
  return code !== undefined && localeCodes.includes(code);
}

export function getUi(locale: string | undefined): Ui {
  return ui[isLocale(locale) ? locale : defaultLocale];
}

/** Site URL of `path` (starting with `/`) in `locale`, honouring the base path. */
export function localePath(locale: string, path = "/"): string {
  const prefix = locale === defaultLocale ? "" : `/${locale}`;
  return `${base}${prefix}${path}`;
}

/** Splits a site pathname into its locale and the locale-free path. */
export function splitPath(pathname: string): { locale: string; path: string } {
  let rest = pathname.startsWith(base) ? pathname.slice(base.length) : pathname;
  if (!rest.startsWith("/")) rest = `/${rest}`;
  const first = rest.split("/")[1];
  if (first !== defaultLocale && isLocale(first)) {
    return { locale: first, path: rest.slice(first.length + 1) || "/" };
  }
  return { locale: defaultLocale, path: rest };
}

/** The same page as `pathname`, in another locale. */
export function switchLocalePath(pathname: string, target: string): string {
  return localePath(target, splitPath(pathname).path);
}
