import { Download, History, Star } from "lucide-react";
import { useI18n } from "fumadocs-ui/contexts/i18n";
import {
  LanguageSelect,
  LanguageSelectText,
  type LanguageSelectProps,
} from "fumadocs-ui/layouts/shared/slots/language-select";
import { defaultLocale, getUi, localePath } from "@/i18n";

const item =
  "flex items-center gap-2 rounded-lg px-2 py-1.5 text-sm text-fd-muted-foreground no-underline transition-colors hover:bg-fd-accent hover:text-fd-accent-foreground";

/**
 * The sidebar footer reads: Downloads, Changelog, language, then one row with
 * GitHub (left) and the theme switch (right). Fumadocs renders the language
 * select first in the footer, so it is the slot that carries the two links;
 * `variant="secondary"` is only passed by the desktop sidebar, the mobile
 * drawer gets the plain select.
 */
function LanguageWithLinks(props: LanguageSelectProps) {
  const { locale = defaultLocale } = useI18n();
  const ui = getUi(locale);
  return (
    <>
      {props.variant === "secondary" && (
        <nav aria-label={ui.nav.main} className="mb-1 flex flex-col gap-0.5">
          <a className={item} href={localePath(locale, "/downloads/")}>
            <Download size={16} aria-hidden="true" />
            {ui.nav.download}
          </a>
          <a className={item} href={localePath(locale, "/changelog/")}>
            <History size={16} aria-hidden="true" />
            {ui.nav.changelog}
          </a>
        </nav>
      )}
      <LanguageSelect {...props} />
    </>
  );
}

export const languageSelectSlot = {
  root: LanguageWithLinks,
  text: LanguageSelectText,
};

/** GitHub with the star count: the content of the left side of the theme row. */
export function GithubRowIcon({
  label,
  stars,
}: {
  label: string;
  stars?: string;
}) {
  return (
    <span className="inline-flex items-center gap-1.5 px-1 text-sm">
      <Star aria-hidden="true" fill="currentColor" />
      <span>{label}</span>
      {stars && <span className="text-xs">{stars}</span>}
    </span>
  );
}
