/** A release-please pull request that has not been merged (and so not published) yet. */
export interface PendingRelease {
  /** Semver version the PR will release, without the `v`. */
  version: string;
  /** Link to the release-please PR. */
  url: string;
  /** Markdown release notes. */
  content: string;
}

const versionPattern = /\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?/;
// `## [1.2.3](compare-url) (date)` or `## 1.2.3 (date)`
const headingPattern = /^##\s+\[?(\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?)\]?.*$/m;

/**
 * Turns the release-please PR (GitHub REST shape) into a pending entry.
 * Returns `undefined` for anything that does not look like one.
 */
export function parsePendingRelease(pr: unknown): PendingRelease | undefined {
  if (typeof pr !== "object" || pr === null) return undefined;
  const { title, html_url: url, body } = pr as Record<string, unknown>;
  if (typeof url !== "string" || !url) return undefined;
  const text = typeof body === "string" ? body.replace(/\r\n/g, "\n") : "";

  const heading = headingPattern.exec(text);
  const version =
    heading?.[1] ??
    (typeof title === "string" ? versionPattern.exec(title)?.[0] : undefined);
  if (!version) return undefined;

  // Notes: after the version heading (or the whole body), without the
  // `:robot:` preamble and the "generated with Release Please" footer.
  let notes = heading ? text.slice(heading.index + heading[0].length) : text;
  notes = notes.replace(/\n---\s*\nThis PR was generated with[\s\S]*$/, "");
  if (!heading) notes = notes.replace(/^:robot:[^\n]*\n---\s*\n/, "");
  return { version, url, content: notes.trim() };
}
