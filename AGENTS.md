# Wren: instructions for coding agents

Wren is an open-source, low-resource desktop browser in Rust on GPUI and
Servo. Not affiliated with or endorsed by the Servo project, Google, or
the Chromium project.

## Read first

- Spec: `docs/superpowers/specs/2026-11-13-wren-browser-design.md`
- Plan: `docs/superpowers/plans/2026-11-13-m0-shell.md`
- Roadmap: `docs/superpowers/plans/2026-11-13-roadmap.md`

Do the task you were given; do not expand scope.

## Workflow

- Local green is the gate: `mise run build`, `mise run lint`, `mise run test`
  must pass before you commit. Install hooks once with `hk install --mise`.
- Features land through short-lived branches and self-merged, squash-merged PRs.
  Small fixes and docs may go straight to `main`.
- Commits are signed. If signing fails, stop and report; never commit unsigned.
- Test first: write the failing test, see it fail, then make it pass.
- Everything you run is a `mise` task; do not invent ad hoc commands.
- PRs use `.github/pull_request_template.md` and fill every section; the
  `pr-template` CI check fails otherwise. Check the body before opening
  the PR with `mise run lint:pr-body -- --body-file <file> --title
  "<title>" --base origin/main`.
- Repo tooling scripts (`scripts/*.ts`) are Bun, tested with `mise run
  test:repo`.

## Conventions

- Crates are unprefixed snake_case under `crates/` (`engine`, `tabs`,
  `store`, `omnibox`, `settings`, `themes`, `components`, `parity`); the
  binary crate is `wren`. Only globally named things keep the `wren_`
  prefix (`WREN_*` env vars, `wren://`).
- Only the `engine` crate imports `servo`. Every other crate depends on
  `engine`'s public API.
- Platform: macOS 14+ on Apple Silicon only. CI runs on `macos-latest`.
- Licence `MPL-2.0`; docs are CC-BY-4.0.
- JavaScript: Bun only (its built-in formatter and type checker). No Node,
  npm or pnpm.
- Never commit tokens, cookies or unredacted captures.

## Agent tooling

- Skills live in `.agents/skills/<name>/SKILL.md`; subagents in
  `.agents/agents/<name>.md` (see `.agents/agents/README.md`).
- `CLAUDE.md`, `.claude/skills` and `.claude/agents` are symlinks. Never
  put real files in vendor paths; an hk check enforces this.
