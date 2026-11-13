<!-- top guidance -->

## Summary

Adds pinning to the conversation header so people keep key messages in reach.

## Plan task or issue

`docs/superpowers/plans/2026-11-12-m3-multi-workspace-and-awareness.md` Task 34

## Type of change (pick one)

<!-- tick one -->

- [x] `feat`: new user-facing feature
- [ ] `fix`: bug fix
- [ ] `chore`: spec, plan, research, tooling

## User-visible change

A pin button in the message actions and a Pins panel in the header.

## Screenshots and visual diff

| Platform | Theme | Before | After |
|---|---|---|---|
| macOS | light | ![](https://example.invalid/b-l.png) | ![](https://example.invalid/a-l.png) |
| macOS | dark | ![](https://example.invalid/b-d.png) | ![](https://example.invalid/a-d.png) |

## Performance

`mise run test:perf`: allocations/event 41 -> 41 (ceiling 49).

## Tests

- [x] Tests added or updated for every behaviour change
- [x] Failing-first evidence (link or paste): commit abc1234
- [x] UI change: view test, scenario and baselines
- [ ] Hot path: ceilings N/A: no budget change

## Docs and translations

- [x] User docs in every locale
- [ ] App catalogs N/A: no new strings

## Checks run locally

- [x] `mise run build`
- [x] `mise run lint`
- [x] `mise run test`

## Security, privacy and confidentiality

- [x] No tokens, cookies, real IDs, real names or unredacted captures
- [x] Core stays provider-agnostic
- [x] No employer named
- [x] No third-party branding

## Breaking changes and migrations

N/A: additive only

## Release note

- [x] The title is a Conventional Commit written for users
