<!--
How to fill this in (the `pr-template` check enforces it; run it locally
with `mise run lint:pr-body -- --body-file <file> --title "<title>" --base origin/main`):

- Keep every `##` heading. Write in each section; HTML comments do not count.
- A section that does not apply says `N/A: <reason>`. A bare `N/A` fails.
- Tick a box with `[x]`, or leave it `[ ]` and end the line with
  `N/A: <reason>`. An unticked box without a reason fails.
- PR title = release-note line, written for users. Conventional Commit:
  `type(scope): summary`, e.g. `feat(components): pin messages in a channel`.
  feat, fix, perf and docs (user docs) show in the changelog; refactor, test,
  ci, build, chore and revert are hidden. Spec, plan and research changes use
  `chore(spec)`, `chore(plan)`, ... `!` before the colon marks a breaking change.
- Confidentiality: never name an employer, its products or internal tools,
  and never paste real workspace data, names or IDs.
-->

## Summary

<!-- Fill this: what changes and why, in two to five sentences. Lead with the user-facing effect. -->

## Plan task or issue

<!-- Fill this: the plan task (e.g. `docs/superpowers/plans/2026-11-12-m2-store-and-rich-messaging.md` Task 28) or issue (#123). Otherwise `N/A: <reason>`. -->

## Type of change (pick one)

<!-- Tick exactly one; it must match the type in the PR title. Leave the others unticked (no N/A needed here). -->

- [ ] `feat`: new user-facing feature
- [ ] `fix`: bug fix
- [ ] `perf`: performance improvement
- [ ] `docs`: user docs (site)
- [ ] `refactor`: no behaviour change
- [ ] `test`: tests only
- [ ] `ci`: CI and workflows
- [ ] `build`: build, packaging, dependencies
- [ ] `chore`: spec, plan, research, tooling
- [ ] `revert`: reverts an earlier change

## User-visible change

<!-- Fill this: what a user sees or can do differently (menus, settings, defaults, text). Otherwise `N/A: <reason>`. -->

## Screenshots and visual diff

<!--
Required when UI files change (components, themes, icons, the wren views,
visual baselines, the site's src/). Give:
1. Before and after screenshots per platform you changed and per theme:

   | Platform | Theme | Before | After |
   |---|---|---|---|
   | macOS | light | ![](url) | ![](url) |
   | macOS | dark | ![](url) | ![](url) |

   Synthetic data only (FakeProvider scenarios); no real workspace content.
2. The visual-diff report from `mise run test:visual` (attach the images or
   link the CI `visual-<os>` artifact: https://github.com/<owner>/<repo>/actions/runs/<id>).
   Baseline PNG changes ride in this PR and are reviewed here.
If files on the UI list changed but nothing renders differently, add a line
`UI-Unchanged: <reason>` and keep `N/A: <reason>` here. No UI files: `N/A: <reason>`.
-->

## Performance

<!--
Required with numbers when a hot path or a budget changes (domain, store,
store_sqlite, components, wren, calls, provider realtime/markup source,
perf_budget.toml). Give the `mise run test:perf` result and, for budget rows,
before and after values against spec section 9 ("Performance budgets"), e.g.
"allocations/event 41 -> 38 (ceiling 49); idle RSS 52 MB -> 52 MB (budget 60)".
A hot-path edit with no perf effect: `Perf-Unaffected: <reason>`.
Otherwise `N/A: <reason>`.
-->

## Tests

<!-- Tick or mark `N/A: <reason>`. Failing-first evidence: the commit, CI run or pasted output showing the new test failing before the fix. -->

- [ ] Tests added or updated for every behaviour change
- [ ] Failing-first evidence (link or paste):
- [ ] UI change: view test, `ui_testing` scenario with visual baselines for every platform, e2e step for a user flow
- [ ] Hot path: `perf_budget.toml` ceilings kept or deliberately changed (say why in Performance)

## Docs and translations

<!-- Tick or mark `N/A: <reason>`. Pure refactors, test-only and CI-only changes may mark these N/A. -->

- [ ] User docs: `site/content/docs/*.mdx` **and** every translation (`*.<locale>.mdx`, `meta.<locale>.json`, `src/i18n`); for a typo or formatting-only English edit add a `Translations-Unchanged: <reason>` line to this body
- [ ] App catalogs (`crates/**/i18n/*.ftl`) updated in every locale
- [ ] Spec or plan updated (`docs/specs/`, `docs/superpowers/plans/`) if behaviour, design or a budget changed
- [ ] Provider docs (`docs/providers/<codename>/`) if endpoints, events or fixtures changed
- [ ] `AGENTS.md` and `CONTRIBUTING.md` if the workflow changed

## Checks run locally

<!-- Tick what you ran and saw pass. Mark the rest `N/A: <reason>` (e.g. "N/A: no site change"). -->

- [ ] `mise run build`
- [ ] `mise run lint` (fmt, clippy, actionlint, typos, gitleaks, site lint)
- [ ] `mise run test`
- [ ] `mise run test:repo` (repo tooling tests)
- [ ] `mise run test:perf` (hot paths or budgets)
- [ ] `mise run test:visual` and `mise run test:e2e` (UI changes)
- [ ] `mise run site:verify` and `bun run format` in `site/` (site or docs)
- [ ] `mise run site:check-translations -- --base origin/main` (site or docs)
- [ ] `mise run recon:check -- <dir>` clean (captures or fixtures)

## Security, privacy and confidentiality

<!-- Every box must be ticked; these never apply as N/A. -->

- [ ] No tokens, cookies, real IDs, real names or unredacted captures
- [ ] Core stays provider-agnostic (no provider names or types outside `crates/providers/` and the `wren` registry)
- [ ] No employer, its products or internal tools named anywhere (code, docs, commits, this PR)
- [ ] No third-party branding, logos, colours, icons, fonts, sounds or screenshots

## Breaking changes and migrations

<!-- Fill this: settings keys renamed or removed (with the migration), store schema migrations, changed defaults, keymap or plugin API changes. Required when the title has `!`. Otherwise `N/A: <reason>`. -->

## Release note

<!-- The PR title is the release-note line. Tick to confirm. -->

- [ ] The title is a Conventional Commit written for users and its type matches the box above
