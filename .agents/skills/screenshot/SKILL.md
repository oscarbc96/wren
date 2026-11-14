---
name: screenshot
description: Capture and check a screenshot of the real Wren window for any visible UI change or any layout or rendering bug.
---

# Screenshot

Unit tests do not show what the window looks like. Use this whenever you
change visible UI, and whenever you investigate a layout or rendering bug.

## Take one

```
SHOT_NAME=<name> mise run shot [URL...]
```

Without mise: `SHOT_NAME=<name> scripts/shot.sh [URL...]`. Both build,
sign and launch Wren with a throwaway profile, wait for the page (default
3 s, more with `--screenshot-delay-ms`), save
`target/shots/<name>.png` and quit. Wren captures its own window, so no
Screen Recording permission is needed.

Use a `data:` URL for a deterministic page, for example
`'data:text/html,<body style="background:%23c33"><h1>Hello</h1></body>'`
(`%23` is `#`).

## Check it

Open the PNG and look at it. Do not report a UI change done from the
command's exit code alone. Check at least:

- the page content is visible (not blank white, not black);
- the web content starts right below the toolbar and fills the window;
- tabs, toolbar and omnibox are not clipped or overlapping the traffic
  lights;
- text is sharp at the display's scale (not tiny or blurry).

State in the PR test plan which screenshots you took and what they show.

## Debugging aids

- `WREN_DEBUG_EVENTS=1` prints every engine event (`UrlChanged`,
  `TitleChanged`, `Load`, ...) to stderr: a page that loads but shows
  blank is a paint problem, not a network one.
- A blank page with a loaded title usually means the Servo surface is
  presented without a new frame or a web view was hidden before its first
  frame (see `crates/engine/src/servo_engine.rs`).
