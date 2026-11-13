# Contributing to Wren

Read the spec at `docs/superpowers/specs/2026-11-13-wren-browser-design.md` and
the plans in `docs/superpowers/plans/` first. `AGENTS.md` has the project rules
in short form.

## CLA

Contributions need a signed CLA (`CLA.md`). CLA Assistant asks you to sign once on
your first pull request. Code under another licence (for example from
Servo or GPUI-CE) must keep its header and be listed in `NOTICE`.

## Workflow: local green is the gate

1. `mise install` then `hk install --mise` (once per clone).
2. Work on a short-lived branch, test first (write the failing test, see it
   fail, make it pass).
3. `mise run build`, `mise run lint` and `mise run test` must pass locally. The
   hk pre-commit and pre-push hooks enforce this.
4. Open a pull request with `.github/pull_request_template.md` filled in (the
   `pr-template` check enforces it; `mise run lint:pr-body` runs it locally).
5. CI re-runs the checks on macOS.

Small fixes and docs may go straight to `main` for the maintainer.

## Releases

Pull request titles become changelog lines, so write them as Conventional
Commits. release-please keeps a Release PR open on `main`; merging it tags
the release (`vX.Y.Z`) and publishes the GitHub Release. Do not create tags
or releases by hand.

## Rules

- Never commit captured traffic, tokens, cookies or real user data.
- JavaScript tooling is Bun only (its built-in formatter and type checker).
- Commits are signed. Use Conventional Commit messages (`feat:`, `fix:`,
  `docs:`, `chore:`).

By participating you agree to the `CODE_OF_CONDUCT.md`.
