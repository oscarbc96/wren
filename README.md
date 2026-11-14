# Wren

Open-source browser with generative UI on top: the AI gets tools to read
and act on web pages with your own logins, and builds pages made for you
(a train comparison you can book from, a dashboard condensing your three
newspapers) that you can save, re-check and turn into routines. Written in
Rust on [GPUI](https://gpui.rs) and [Servo](https://servo.org).

Pre-alpha. Design spec: `docs/superpowers/specs/2026-11-13-wren-browser-design.md`.
Implementation plans: `docs/superpowers/plans/`.
Roadmap: `docs/superpowers/plans/2026-11-13-roadmap.md`.

Site: https://oscarbc96.github.io/wren/
Repository: https://github.com/oscarbc96/wren

## Building from source

Requires macOS 14+ on Apple Silicon, Rust 1.100, and the tools in `mise.toml`.

```
mise install
mise run build
mise run run
```

## Licence

Code is licensed under `MPL-2.0`. Docs are `CC-BY-4.0`. See
`LICENSE`, `NOTICE` and `TRADEMARKS.md`.
