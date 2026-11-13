# Vendored dependencies

Each crate here is a copy of the crates.io release with a minimal patch,
wired in through `[patch.crates-io]` in the root `Cargo.toml`. LICENSE files
are kept. Drop a copy once upstream ships the fix.

- `zed-font-kit`: its non-macOS `freetype-sys` pin is raised to `0.23` so
  it does not clash with Servo's `freetype-sys` (`links = "freetype"`). The
  code path is compiled out on macOS.
- `stylo_derive` 0.23.0: the `ToCss` derive emitted bare `Ok(())` values
  that were then used with `?`. GPUI-CE turns on `log`'s `kv` features,
  which add another `From<_> for fmt::Error` impl, so rustc can no longer
  infer the error type (E0282). The patch writes
  `Ok::<(), std::fmt::Error>(())` in `to_css.rs`.
- `gpui_ce_macos` 0.3.0: `did_finish_launching` ran the app's launch
  callback while holding the platform mutex, so `open_window` at startup
  deadlocked. The patch takes the callback out of the lock first, as
  upstream main already does.
