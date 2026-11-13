import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";
import { defaultLocale, getUi, localePath } from "@/i18n";

const repoUrl = "https://github.com/oscarbc96/wren";

/**
 * `stars` is the compact star count fetched at build time. Without it the
 * nav keeps the plain GitHub icon link.
 */
export function baseOptions(
  locale: string = defaultLocale,
  stars?: string,
): BaseLayoutProps {
  const ui = getUi(locale);
  return {
    nav: {
      title: <span className="wren-wordmark">Wren</span>,
      url: localePath(locale, "/"),
    },
    // Light, dark and system; the provider defaults to system (see provider.tsx).
    themeSwitch: { mode: "light-dark-system" },
    ...(stars ? {} : { githubUrl: repoUrl }),
    links: [
      {
        text: ui.nav.docs,
        url: localePath(locale, "/docs/"),
        active: "nested-url",
      },
      {
        text: ui.nav.changelog,
        url: localePath(locale, "/changelog/"),
        active: "nested-url",
      },
      ...(stars
        ? [
            {
              type: "button" as const,
              text: `${ui.nav.github} ★ ${stars}`,
              url: repoUrl,
              external: true,
              secondary: true,
            },
          ]
        : []),
    ],
  };
}
