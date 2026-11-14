/**
 * Pending (unreleased) changes for the changelog page. Pure helpers only: the
 * network code lives in `github.ts`. Nothing here may throw on odd input.
 */

export interface Unreleased {
  /** Proposed next version (`0.2.0`), when known. */
  version?: string;
  /** Markdown release notes. */
  notes: string;
  /** Where the notes came from. */
  source: "release-pr" | "commits";
  /** Link to the release PR, when there is one. */
  url?: string;
}

export interface ParsedSubject {
  type: string;
  scope?: string;
  breaking: boolean;
  description: string;
}

const subjectRe = /^([a-z]+)(?:\(([^)]*)\))?(!)?:\s+(\S.*)$/;

/** Parse a Conventional Commit subject; `undefined` when it is not one. */
export function parseSubject(subject: string): ParsedSubject | undefined {
  const match = subjectRe.exec(subject.trim());
  if (!match) return undefined;
  const [, type, scope, bang, description] = match;
  return {
    type: type!,
    scope: scope || undefined,
    breaking: bang === "!",
    description: description!,
  };
}

/** Section titles, in release-please order. `docs` and friends stay hidden. */
const sections: [type: string, title: string][] = [
  ["feat", "Features"],
  ["fix", "Bug Fixes"],
  ["perf", "Performance Improvements"],
  ["revert", "Reverts"],
];

const breakingTitle = "Breaking Changes";

function item(parsed: ParsedSubject, repoUrl: string): string {
  const description = parsed.description.replace(
    /\(#(\d+)\)\s*$/,
    (_, n: string) => `([#${n}](${repoUrl}/pull/${n}))`,
  );
  return parsed.scope
    ? `* **${parsed.scope}:** ${description}`
    : `* ${description}`;
}

/**
 * Group commit subjects like release-please: features, fixes, performance and
 * reverts, plus any breaking change; chore, ci, test, build, refactor and docs
 * are hidden unless breaking. Returns Markdown, empty when nothing qualifies.
 */
export function groupSubjects(subjects: string[], repoUrl: string): string {
  const groups = new Map<string, string[]>();
  const add = (title: string, line: string) => {
    const list = groups.get(title);
    if (list) list.push(line);
    else groups.set(title, [line]);
  };
  for (const subject of subjects) {
    const parsed = parseSubject(subject);
    if (!parsed) continue;
    const line = item(parsed, repoUrl);
    if (parsed.breaking) add(breakingTitle, line);
    const section = sections.find(([type]) => type === parsed.type);
    if (section && !parsed.breaking) add(section[1], line);
  }
  const order = [breakingTitle, ...sections.map(([, title]) => title)];
  return order
    .filter((title) => groups.has(title))
    .map((title) => `### ${title}\n\n${groups.get(title)!.join("\n")}`)
    .join("\n\n");
}

/** `chore(main): release 0.2.0` or a `## [0.2.0](...)` heading to `0.2.0`. */
export function parseVersion(text: string): string | undefined {
  return /\b(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)\b/.exec(text)?.[1];
}

/**
 * Turn a release-please PR into notes: drop its robot preamble, the
 * `## [version](...) (date)` heading and the generator footer.
 */
export function parseReleasePr(pr: {
  title?: string | null;
  body?: string | null;
  html_url?: string | null;
}): Unreleased | undefined {
  let body = (pr.body ?? "").replace(/\r\n/g, "\n");
  const footer = body.search(/^This PR was generated with/m);
  if (footer >= 0) body = body.slice(0, footer);
  let version = parseVersion(pr.title ?? "");
  const heading = /^##\s+(.*)$/m.exec(body);
  if (heading && /\d+\.\d+\.\d+/.test(heading[1]!)) {
    version ??= parseVersion(heading[1]!);
    body = body.replace(heading[0], "");
  }
  const notes = body
    .split("\n")
    .filter((line) => line.trim() !== "---" && !/^(🤖|:robot:)/.test(line))
    .join("\n")
    .trim();
  if (!notes && !version) return undefined;
  return {
    version,
    notes,
    source: "release-pr",
    url: pr.html_url || undefined,
  };
}

/** Notes from the commits on main since the last release; `undefined` when empty. */
export function fromCommits(
  subjects: string[],
  repoUrl: string,
): Unreleased | undefined {
  const notes = groupSubjects(subjects, repoUrl);
  return notes ? { notes, source: "commits" } : undefined;
}
