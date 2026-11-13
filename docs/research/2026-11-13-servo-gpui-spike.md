# Servo inside a window: spike (M0 Task 2)

Date: 2026-11-13. Worktree: spike/servo-in-gpui.

## Prerequisites (macOS, Apple Silicon)

- Rust toolchain (edition 2024). Run builds through the repo's Rust toolchain.
- Python >= 3.11 for Servo's `servo-script-bindings` codegen. `mise.toml` pins `python = "3.14"`.
- No GStreamer. The `media-gstreamer` feature is NOT enabled; Servo's default bundled media path is used.
- Cargo target dir `~/.cache/wren-target` (preset). Do not change it.

## Build

```
cargo build -p wren --examples
cargo run -p wren --example servo_in_winit -- --url https://example.org --exit-after 20
cargo run -p wren --example servo_in_gpui -- --url https://example.org --exit-after 20
cargo run -p wren --example servo_in_gpui -- --two --exit-after 25
```

## Exact Servo API names used

### Both examples

- Builder: `servo::ServoBuilder::default().event_loop_waker(Box::new(waker)).build()` and `servo.setup_logging()`.
- Event loop waker: `embedder_traits::EventLoopWaker` trait (`clone_box`, `wake`).
- Rendering context: `servo::WindowRenderingContext::new(display_handle, window_handle, size)` from Servo, made current with `make_current()`, resized with `resize(PhysicalSize<u32>)`, presented with `present()`. Passed to `WebViewBuilder` as `Rc<WindowRenderingContext>`.
- WebView: `servo::WebViewBuilder::new(&servo, rendering_context).url(url).delegate(delegate).build()`, plus `.hide()` / `.show()`, `.resize(PhysicalSize<u32>)`, `.paint()`.
- Delegate: `servo::WebViewDelegate` trait. Implemented methods: `notify_url_changed`, `notify_page_title_changed`, `notify_load_status_changed`, `notify_new_frame_ready`, `notify_crashed`.
- Load state: `servo::LoadStatus` (`Started`, `HeadParsed`, `Complete`, `Stopped`).

### `servo_in_winit.rs`

- Host: `winit` 0.30 (`EventLoop`, `ApplicationHandler`, `Window`, `WindowAttributes`, `WindowEvent`).
- Handles: `winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle}` from the winit window.
- Waker: wraps `winit::event_loop::EventLoopProxy<WakerEvent>` and wakes via `send_event`.

### `servo_in_gpui.rs`

- Host: `gpui` (gpui-ce 0.3.0) `Application` via `gpui_platform::application()` and `App::run` / `open_window`.
- GPUI window: `WindowOptions::default().window_bounds(gpui::WindowBounds::Windowed(gpui::Bounds::centered(...)))`.
- Native parent view: `<gpui::Window as raw_window_handle::HasWindowHandle>::window_handle(window)` to get the GPUI window's raw `AppKitWindowHandle`, extract `ns_view`, and cast to `objc2_app_kit::NSView`.
- Child view: `objc2_app_kit::NSView` created with `NSView::initWithFrame(NSView::alloc(MainThreadMarker), frame)` (objc2 0.6), configured for width/height resizing autoresizing mask, and added as subview of the GPUI window view.
- Servo handles: `raw_window_handle::AppKitWindowHandle::new` / `RawWindowHandle::AppKit` / `RawDisplayHandle::AppKit(AppKitDisplayHandle::new())`, borrowed with `raw_window_handle::{DisplayHandle, WindowHandle}::borrow_raw` for `WindowRenderingContext::new`.
- Driving Servo: a GPUI foreground task (`cx.spawn` + `background_executor().timer(8 ms)`) calls `WindowHandle::update` -> `RootView::tick`, which runs `servo.spin_event_loop()`, resizes, `webview.paint()` and `rendering_context.present()`. Calling `window.refresh()` from inside `render` does not schedule another frame in GPUI, so Servo must be ticked from outside `render`. The waker is a no-op here; `ServoEngine` (Task 4B) should make `wake` signal the foreground task through a channel instead of polling every 8 ms.
- Layout: a simple GPUI header (`"Wren Servo spike"`, fixed height) is rendered above a flex spacer; the child NSView frame is kept in sync each render to the GPUI window content bounds below the header, and Servo `rendering_context`/`WebView` sizes are resized on pixel-size changes.

## Approach that worked

The child-NSView approach works in this environment: Servo renders into an `NSView` added as a child of the GPUI-CE window's native view, using `WindowRenderingContext` with raw-window-handle AppKit handles for that child view. GPUI renders a minimal header element while Servo owns the content area via the native child view. No offscreen fallback was required.

## What works

- GPUI-CE opens a window and hosts a child NSView below a simple header.
- Servo renders into the child view via `WindowRenderingContext`.
- Delegate events are printed: `URL_CHANGED <url>`, `TITLE <title>`, `LOAD_STARTED`, `LOAD_COMPLETE`, `CRASHED <reason>`.
- `--url <URL>` and `--exit-after <secs>` are supported; at exit prints `RSS_BYTES=<n>` (sysinfo, own process).
- `--two` creates two webviews: first loads `--url`, second loads `https://servo.org` (hidden), proving multiple webviews; only the first is shown.
- Resize handling keeps the child view frame and Servo rendering context/webview pixel sizes in sync with the GPUI content area below the header.

## What is missing / limitations

- Input forwarding is minimal and incomplete: the GPUI header receives GPUI events, but mouse move/click, scroll, and keyboard input are not comprehensively forwarded from the GPUI window/child view into Servo `WebView` input APIs. Further work is needed to translate macOS/GPUI input events to Servo input events for interactive browsing.
- The example polls Servo every 8 ms instead of waking on `EventLoopWaker::wake` (costs idle CPU; fix in Task 4B).
- Crash behaviour was not exercised with a crashing page; with per-tab processes (spec 7.2) it becomes a process kill test in Task 4B.

## Measurements (debug build, this machine)

- `servo_in_winit`, example.org: RSS about 179 MB (`RSS_BYTES=178946048`).
- `servo_in_gpui`, example.org, 12 to 15 s: RSS about 182 to 185 MB; frame read back from the child view before present: 800x604, 11262 dark (text) pixels, so Servo really draws into the child NSView.
- `servo_in_gpui --two` (example.org + servo.org, second hidden): RSS about 253 MB.
- Clean build of Servo: about 10 minutes.

## GPUI-CE 0.3.0 launch deadlock

`gpui_ce_macos` 0.3.0 calls the app launch callback while still holding its platform mutex (`if let Some(callback) = platform.0.lock().finish_launching.take() { callback(); }`), so any `open_window` during launch deadlocks; even a minimal hello-window hangs. Upstream main already takes the callback out of the lock first. `vendor/gpui_ce_macos` carries that one-line fix until a release ships it.

## Prior notes (winit spike)

- Earlier spike hosted Servo in a `winit` window (`servo_in_winit.rs`) because combining Servo and GPUI-CE had resolver/link conflicts at that time. The freetype and sqlite conflicts noted previously were addressed in the workspace/vendor patches already present on this branch; `cargo build -p wren --examples` now builds both examples together.

## Files

- `crates/wren/examples/servo_in_winit.rs`
- `crates/wren/examples/servo_in_gpui.rs`
- `docs/research/2026-11-13-servo-gpui-spike.md`
