import type { GitHubChangelogEntry } from "@fumadocs/changelog/github";
import Link from "fumadocs-core/link";
import { createMarkdownRenderer } from "fumadocs-core/content/md";
import { remarkGfm } from "fumadocs-core/mdx-plugins/remark-gfm";
import { rehypeCode } from "fumadocs-core/mdx-plugins/rehype-code";
import defaultMdxComponents from "fumadocs-ui/mdx";
import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react";
import { type ComponentProps, type ReactNode, Suspense } from "react";
import { cn } from "../lib/cn";

export interface ChangelogTimelineEntry extends Omit<
  GitHubChangelogEntry,
  "prerelease"
> {
  prerelease?: boolean;
  /** rendered content, replaces `content` */
  body?: ReactNode;
}

const { Markdown } = createMarkdownRenderer({
  remarkPlugins: [remarkGfm],
  // `tab` wraps code blocks in MDX elements, which Markdown can't render
  rehypePlugins: [[rehypeCode, { fallbackLanguage: "plaintext", tab: false }]],
});

const mdxComponents = {
  ...defaultMdxComponents,
  img: "img" as const,
};

export interface ChangelogLabels {
  /** BCP 47 tag used to format dates */
  dateLocale: string;
  prerelease: string;
  newer: string;
  older: string;
}

function formats(locale: string) {
  return {
    month: new Intl.DateTimeFormat(locale, {
      month: "long",
      year: "numeric",
      timeZone: "UTC",
    }),
    day: new Intl.DateTimeFormat(locale, {
      month: "short",
      day: "numeric",
      timeZone: "UTC",
    }),
  };
}

export function ChangelogTimeline({
  entries,
  labels,
}: {
  entries: ChangelogTimelineEntry[];
  labels: ChangelogLabels;
}) {
  const { month: monthFormat } = formats(labels.dateLocale);
  const months = new Map<string, ChangelogTimelineEntry[]>();
  for (const entry of [...entries].sort(
    (a, b) => b.date.getTime() - a.date.getTime(),
  )) {
    const month = monthFormat.format(entry.date);
    const items = months.get(month);
    if (items) items.push(entry);
    else months.set(month, [entry]);
  }

  return (
    <div className="flex flex-col gap-8">
      {Array.from(months, ([month, items]) => (
        <section key={month}>
          <h2 className="sticky top-14 z-1 bg-fd-background/80 py-4 text-sm font-medium text-fd-muted-foreground backdrop-blur-sm">
            {month}
          </h2>
          <ol className="ms-3 border-s ps-6">
            {items.map((entry) => (
              <ChangelogItem key={entry.id} entry={entry} labels={labels} />
            ))}
          </ol>
        </section>
      ))}
    </div>
  );
}

function ChangelogItem({
  entry,
  labels,
}: {
  entry: ChangelogTimelineEntry;
  labels: ChangelogLabels;
}) {
  const dayFormat = formats(labels.dateLocale).day;
  const versions = entry.versions?.filter(
    ({ name, version }) => `${name}@${version}` !== entry.title,
  );

  return (
    <li id={entry.id} className="scroll-mt-28 pb-10 last:pb-0">
      <div className="mb-2 flex flex-wrap items-center gap-1.5">
        <time
          dateTime={entry.date.toISOString()}
          className="relative pe-2 text-xs font-medium text-fd-primary tabular-nums"
        >
          <span className="absolute -start-6 top-1/2 size-2 -translate-x-1/2 -translate-y-1/2 rounded-full bg-fd-primary rtl:translate-x-1/2" />
          {dayFormat.format(entry.date)}
        </time>
        {versions?.map(({ name, version }) => (
          <span
            key={name}
            className="rounded-md border bg-fd-secondary px-1.5 py-0.5 font-mono text-xs text-fd-secondary-foreground"
          >
            {name}@{version}
          </span>
        ))}
        {entry.prerelease && (
          <span className="rounded-md border border-fd-primary/40 bg-fd-primary/10 px-1.5 py-0.5 text-xs font-medium text-fd-primary">
            {labels.prerelease}
          </span>
        )}
      </div>
      <h3 className="mb-4 font-semibold">
        {entry.url ? (
          <a href={entry.url} className="hover:text-fd-primary">
            {entry.title}
          </a>
        ) : (
          entry.title
        )}
      </h3>
      <div className="prose prose-sm text-fd-muted-foreground">
        {entry.body ??
          (entry.content && (
            <Suspense>
              <Markdown async components={mdxComponents}>
                {entry.content}
              </Markdown>
            </Suspense>
          ))}
      </div>
    </li>
  );
}

/** links to the pages of newer and older entries */
export function ChangelogPagination({
  newer,
  older,
  labels,
  className,
  ...props
}: ComponentProps<"nav"> & {
  newer?: string;
  older?: string;
  labels: ChangelogLabels;
}) {
  if (!newer && !older) return;
  const linkClass =
    "inline-flex items-center gap-1.5 rounded-lg border bg-fd-card px-3 py-1.5 text-sm font-medium transition-colors hover:bg-fd-accent hover:text-fd-accent-foreground";

  return (
    <nav {...props} className={cn("flex flex-row", className)}>
      {newer && (
        <Link href={newer} className={linkClass}>
          <ChevronLeftIcon className="size-4 rtl:rotate-180" />
          {labels.newer}
        </Link>
      )}
      {older && (
        <Link href={older} className={cn(linkClass, "ms-auto")}>
          {labels.older}
          <ChevronRightIcon className="size-4 rtl:rotate-180" />
        </Link>
      )}
    </nav>
  );
}
