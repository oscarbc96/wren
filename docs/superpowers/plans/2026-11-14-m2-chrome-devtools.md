# M2 Chrome DevTools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wren speaks the Chrome DevTools Protocol (CDP): the owner opens
Chrome's DevTools frontend against a Wren tab, and Playwright and
Puppeteer drive Wren through `connectOverCDP` / `puppeteer.connect`.

**Architecture:** a new `cdp` crate holds the protocol layer: message
types, the HTTP discovery endpoints, WebSocket sessions, Target session
routing and the domain handlers. It starts from Obscura's `obscura-cdp`
(Apache-2.0, attributed in `NOTICE` and file headers), keeping its
protocol and session code and replacing its DOM/V8 calls with a
`CdpBackend` trait. `cdp` never imports `servo` or `engine`; the `wren`
binary implements `CdpBackend` over the `Engine` trait and the tab list
in `cdp_bridge.rs`. The server runs on its own thread with a
single-threaded tokio runtime; commands reach the GPUI main thread
through a channel and are pumped in the existing `tick`. DOM, Network and
Console data come from page JavaScript (`Engine::evaluate`) and engine
events, so later Servo patches can replace them without touching `cdp`.

**Tech Stack:** as M1, plus `tokio` (rt, net, sync), `tokio-tungstenite`,
`serde_json`, `uuid`, `base64`; dev: `playwright-core` and
`puppeteer-core` run by Bun.

**Spec:** `docs/superpowers/specs/2026-11-13-wren-browser-design.md`
(sections 7.6, 9 licence table, 10 M2, 11).
Roadmap: `docs/superpowers/plans/2026-11-13-roadmap.md`.

## Global Constraints

- All M0 and M1 constraints hold (MPL-2.0, only `engine` imports `servo`,
  macOS 14+ Apple Silicon, signed squash PRs, local green).
- CDP is **off by default**; enabled per session with
  `--remote-debugging-port <PORT>` (Chrome's flag name) or the setting
  `cdp.enabled` (default `false`), port `cdp.port` (default `9222`).
- The server binds `127.0.0.1` only. It rejects WebSocket upgrades whose
  `Origin` is set and is not `http://127.0.0.1:<port>`,
  `http://localhost:<port>` or `wren://devtools` (DNS rebinding guard),
  and HTTP requests whose `Host` is not `127.0.0.1:<port>` or
  `localhost:<port>`.
- A visible toolbar indicator ("CDP :9222") shows while the server runs.
- Domain order (spec 7.6): Target, Page, Runtime, DOM, Network, Input,
  Console; CSS, Debugger, Performance only as data allows (stubs that
  return `{}` for `enable` are fine).
- Obscura code keeps its Apache-2.0 header and gets "Modified for Wren"
  lines; `NOTICE` lists it. Chrome DevTools frontend (BSD-3-Clause) ships
  with its `LICENSE`.
- No Node, npm or pnpm: frontend fetch and client tests are Bun.
- Unknown CDP methods answer error `-32601` "'<method>' wasn't found",
  never drop the request (clients hang on silence).

## Review Focus

1. **A client that sends a method we do not implement** must get a
   `-32601` error with the same `id` and `sessionId`, not silence.
   Pinned by `unknown_method_errors_with_same_id` (Task 1).
2. **A WebSocket from a web page origin** (`https://evil.example`) must
   be refused, so a site cannot drive the browser. Pinned by
   `foreign_origin_upgrade_refused` (Task 1).
3. **Closing a tab in the UI while a client is attached** must send
   `Target.detachedFromTarget` and `Target.targetDestroyed` and keep the
   connection open. Pinned by `ui_close_detaches_session` (Task 3).
4. **`Runtime.evaluate` on a hung or crashed tab** must answer with an
   error within 30 s, not block the server. Pinned by
   `evaluate_on_crashed_tab_errors` (Task 4).
5. **A CDP-created tab** (`Target.createTarget`) must be a normal tab:
   visible in the tab strip, closable, and covered by sleep rules except
   that tabs with an attached session never sleep. Pinned by
   `attached_tab_never_sleeps` (Task 3).

---

### Task 1: `cdp` crate: protocol, discovery and sessions

**Files:**
- Create: `crates/cdp/{Cargo.toml,src/lib.rs,src/types.rs,src/server.rs,src/http.rs,src/session.rs,src/backend.rs}`
- Create: `crates/cdp/tests/server.rs`
- Modify: workspace `Cargo.toml`, `NOTICE`, `deny.toml` if needed

**Interfaces:**
- Produces:
  - `types.rs` (from Obscura `types.rs`): `CdpRequest { id: u64, method: String, params: Value, session_id: Option<String> }`, `CdpResponse::{success, error}`, `CdpEvent::{new, with_session}`.
  - `backend.rs`:
    ```rust
    pub struct TargetInfo { pub target_id: String, pub url: String, pub title: String, pub attached: bool }
    pub enum BackendEvent { TargetCreated(TargetInfo), TargetChanged(TargetInfo), TargetDestroyed(String), Domain { target_id: String, method: String, params: Value } }
    #[async_trait::async_trait(?Send)]
    pub trait CdpBackend {
        async fn targets(&self) -> Vec<TargetInfo>;
        async fn call(&self, target_id: Option<&str>, method: &str, params: Value) -> Result<Value, CdpFailure>;
        fn events(&self) -> tokio::sync::broadcast::Receiver<BackendEvent>;
    }
    pub struct CdpFailure { pub code: i64, pub message: String }
    ```
  - `server.rs`: `pub struct CdpServer; impl CdpServer { pub fn start(port: u16, backend: Arc<dyn CdpBackendFactory>) -> anyhow::Result<CdpHandle> }`, `CdpHandle::{port(), stop()}`. Runs a dedicated thread with a current-thread tokio runtime and a `LocalSet`.
  - HTTP (`http.rs`): `GET /json/version` (`Browser: "Wren/<version>"`, `Protocol-Version: "1.3"`, `webSocketDebuggerUrl: ws://127.0.0.1:<port>/devtools/browser/<uuid>`), `GET /json` and `/json/list` (one entry per target: `id`, `type: "page"`, `url`, `title`, `webSocketDebuggerUrl`, `devtoolsFrontendUrl: wren://devtools/inspector.html?ws=127.0.0.1:<port>/devtools/page/<id>`), `PUT /json/new?<url>`, `GET /json/close/<id>`.
  - `session.rs`: browser endpoint plus flat sessions (`Target.attachToTarget { flatten: true }` returns `sessionId`; requests with `sessionId` route to that target), and per-page endpoints `/devtools/page/<id>`.
  - Target domain handled in `cdp` itself: `setDiscoverTargets` (emit `targetCreated` for existing targets), `getTargets`, `attachToTarget`, `detachFromTarget`, `setAutoAttach` (attach existing pages, emit `attachedToTarget` with `waitingForDebugger: false`), `getTargetInfo`, `createTarget`/`closeTarget` (forwarded to backend), `Browser.getVersion`, `Browser.close` (forwarded).

- [ ] **Step 1: Write failing tests** in `crates/cdp/tests/server.rs` with a `FakeBackend` (two targets, records calls):
  - `json_version_lists_browser_ws` (GET `/json/version`, `Browser` starts with `"Wren/"`, ws URL has `/devtools/browser/`).
  - `json_list_has_two_pages` (two entries, `type == "page"`).
  - `attach_flatten_routes_by_session` (attach to target A, send `Runtime.evaluate` with that `sessionId`, FakeBackend saw `call(Some("A"), "Runtime.evaluate", ..)`; the reply carries the same `sessionId`).
  - `unknown_method_errors_with_same_id` (FakeBackend returns `CdpFailure{code:-32601}`; client gets `{"id":7,"error":{"code":-32601,..}}`).
  - `foreign_origin_upgrade_refused` (upgrade with `Origin: https://evil.example` gets HTTP 403).
  - `bad_host_header_refused` (`Host: evil.example:9222` gets 403).
  - `discover_targets_emits_created` (after `setDiscoverTargets {discover:true}`, two `Target.targetCreated` events).
- [ ] **Step 2: Run** `cargo nextest run -p cdp`. Expected: FAIL (crate missing).
- [ ] **Step 3: Implement.** Copy Obscura's `types.rs` and the accept/upgrade/session parts of `server.rs` and `domains/target.rs` from `https://github.com/h4ckf0r0day/obscura` (commit `0d9a2f764ffdb097269da048002c593865e83a9e`), keep the Apache header, add "Modified for Wren" and strip everything tied to `obscura-browser`, `obscura-js`, cookies, stealth and the proxy. Keep the connection cap (128) and bounded queues.
- [ ] **Step 4: Run** `cargo nextest run -p cdp`. Expected: PASS. `mise run lint` clean, `cargo deny check` clean.
- [ ] **Step 5: Commit** `feat(cdp): protocol server, discovery and target sessions`

### Task 2: Engine hooks for CDP

**Files:**
- Modify: `crates/engine/src/{api.rs,fake.rs,servo_engine.rs}`
- Test: `crates/engine/tests/fake_cdp_hooks.rs`

**Interfaces:**
- Produces on `Engine` (default impls reply with an error so other engines compile):
  - `fn evaluate(&mut self, id: WebViewId, js: &str, reply: EvalReply)` where `pub type EvalReply = Box<dyn FnOnce(Result<serde_json::Value, String>)>`. Servo: `WebView::evaluate_javascript`, mapping `JSValue` to JSON (`Undefined` -> `null`, objects/arrays recursively, element/frame handles -> `{"__wren_node": true}`); JS exceptions -> `Err(message)`.
  - `fn screenshot(&mut self, id: WebViewId, reply: ScreenshotReply)` with `pub type ScreenshotReply = Box<dyn FnOnce(Result<Vec<u8>, String>)>` returning PNG bytes (Servo `WebView::take_screenshot`, encode with `image`).
  - `EngineEvent::Console { id: WebViewId, level: ConsoleLevel, message: String }`, `ConsoleLevel::{Log, Info, Warn, Error, Debug}` (Servo `show_console_message`).
  - `EngineEvent::Request { id: WebViewId, request_id: u64, url: Url, method: String, is_document: bool }` (from `load_web_resource`, after the M1 request filter; blocked requests are not reported).
  - `FakeEngine`: `set_eval_result(id, Result<Value, String>)`, `push_console(id, level, msg)`, screenshot returns a 1x1 PNG.

- [ ] **Step 1: Write failing tests** `fake_evaluate_replies`, `fake_screenshot_is_png` (bytes start with `\x89PNG`), `fake_console_event_drains`.
- [ ] **Step 2: Run** `cargo nextest run -p engine`. Expected: FAIL.
- [ ] **Step 3: Implement** in all three files. A reply that never comes (crashed webview) must still fire: on `Crashed` or `close`, fail pending replies for that view with `Err("target crashed")`.
- [ ] **Step 4: Run** `cargo nextest run -p engine` and add `servo_evaluate_returns_json` gated like the other Servo tests (`#[ignore]` unless `WREN_SERVO_TESTS=1`); run it once locally: `evaluate("1+1")` -> `2`, `evaluate("({a:[1,'x']})")` -> `{"a":[1,"x"]}`.
- [ ] **Step 5: Commit** `feat(engine): evaluate with result, screenshots, console and request events`

### Task 3: CDP bridge in `wren`: server lifecycle, targets and tabs

**Files:**
- Create: `crates/wren/src/cdp_bridge.rs`
- Modify: `crates/wren/src/{app.rs,window.rs,cli.rs,layout.rs}`, `crates/settings/` (schema + snapshot), `crates/tabs/` (sleep exemption)
- Test: `crates/wren/tests/cdp_bridge.rs`

**Interfaces:**
- Consumes: `cdp::{CdpServer, CdpBackend, BackendEvent, TargetInfo}` (Task 1), `Engine::evaluate` (Task 2).
- Produces:
  - `pub struct CdpBridge` holding a `std::sync::mpsc` command queue drained in the window `tick`; `CdpBridge::pump(&mut self, engine: &mut dyn Engine, tabs: &mut Tabs)`.
  - Target ids are `format!("{:032X}", tab_id.0)` (stable per run, 32 hex chars like Chrome).
  - `Target.createTarget {url}` opens a foreground tab; `closeTarget` closes it; `Browser.close` quits the app.
  - Settings: `cdp.enabled: bool = false`, `cdp.port: u16 = 9222`. CLI: `--remote-debugging-port <PORT>` (overrides both for this run).
  - `Tabs::set_attached(id, bool)`; attached tabs never sleep.
  - Toolbar indicator "CDP :<port>" while running.

- [ ] **Step 1: Write failing tests** with `FakeEngine`:
  - `targets_mirror_tabs` (two tabs -> two `TargetInfo` with their URLs).
  - `create_target_opens_tab` and `close_target_closes_tab`.
  - `ui_close_detaches_session` (attached session, close the tab through the tab strip action, client gets `Target.detachedFromTarget` then `Target.targetDestroyed`, connection stays open: a following `Target.getTargets` answers).
  - `attached_tab_never_sleeps` (sleep timer elapses, attached tab still live).
  - `cdp_off_by_default` (no flag, no setting: nothing listens on 9222).
- [ ] **Step 2: Run** `cargo nextest run -p wren`. Expected: FAIL.
- [ ] **Step 3: Implement.** The bridge holds no engine reference off the main thread: the server thread sends `(target_id, method, params, oneshot::Sender)` and the main thread answers.
- [ ] **Step 4: Run** tests; `mise run shot -- --remote-debugging-port 9222 https://example.com`, look at the PNG: the indicator "CDP :9222" is visible in the toolbar. `curl -s 127.0.0.1:9222/json/list` lists the tab.
- [ ] **Step 5: Commit** `feat: cdp server behind --remote-debugging-port`

### Task 4: Page, Runtime, Input and Console domains

**Files:**
- Create: `crates/wren/src/cdp_domains/{mod.rs,page.rs,runtime.rs,input.rs,console.rs}`
- Test: `crates/wren/tests/cdp_domains.rs`

**Interfaces:**
- Consumes: `Engine::{load, reload, back, forward, evaluate, screenshot, input}`, `EngineEvent::{Load, UrlChanged, TitleChanged, Console}`.
- Produces (method -> behaviour):
  - Page: `enable`, `navigate {url}` -> `{frameId, loaderId}`, `reload`, `navigateToHistoryEntry`, `getNavigationHistory`, `getFrameTree` (one main frame, `frame.id == target_id`), `captureScreenshot {format:"png"}` -> base64, `setLifecycleEventsEnabled`, `addScriptToEvaluateOnNewDocument` (stored, run on each `Load::Started` via `evaluate`), `createIsolatedWorld` (returns a new context id, same world for now). Events: `frameStartedLoading`, `frameNavigated`, `domContentEventFired`, `loadEventFired`, `lifecycleEvent {name:"init"|"DOMContentLoaded"|"load"|"networkIdle"}`, `frameStoppedLoading`.
  - Runtime: `enable` (emit `executionContextCreated` with `auxData {isDefault:true, frameId}`; re-emit after each navigation with a new id, `executionContextsCleared` before), `evaluate {expression, returnByValue, awaitPromise}`, `callFunctionOn {functionDeclaration, arguments, returnByValue}` (by value; object ids map to a page-side registry `window.__wrenObjects`), `getProperties` (own enumerable, by value), `releaseObject`, `runIfWaitingForDebugger`. Remote objects: `{type, value}` for by-value, `{type:"object", objectId}` otherwise. Exceptions -> `exceptionDetails`.
  - Input: `dispatchMouseEvent` (`mouseMoved|mousePressed|mouseReleased|mouseWheel`, CSS px == points at scale 1), `dispatchKeyEvent` (`keyDown|keyUp|char`), `insertText` (one `char` per scalar).
  - Console / `Runtime.consoleAPICalled` from `EngineEvent::Console`; `Log.enable` stub.
  - Every call has a 30 s timeout -> error `-32000 "timeout"`.

- [ ] **Step 1: Write failing tests**: `navigate_emits_load_events_in_order`, `evaluate_by_value`, `evaluate_exception_details`, `call_function_on_with_args`, `capture_screenshot_base64_png`, `console_api_called_forwarded`, `mouse_pressed_maps_to_engine_input`, `evaluate_on_crashed_tab_errors` (FakeEngine emits `Crashed` while an evaluate waits: error arrives, server answers the next request).
- [ ] **Step 2: Run** `cargo nextest run -p wren cdp_domains`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** tests; by hand `wren --remote-debugging-port 9222` then send `Runtime.evaluate {expression:"document.title"}` with `bunx wscat` replaced by a 10-line Bun script in `scripts/cdp-eval.ts`; it prints the title.
- [ ] **Step 5: Commit** `feat(cdp): page, runtime, input and console domains`

### Task 5: DOM and Network domains (JS-backed)

**Files:**
- Create: `crates/wren/src/cdp_domains/{dom.rs,network.rs,emulation.rs}`, `crates/wren/src/cdp_domains/dom_agent.js`
- Test: `crates/wren/tests/cdp_dom_network.rs`

**Interfaces:**
- `dom_agent.js` is injected on demand and keeps `window.__wrenNodes` (node id <-> node, `WeakRef` where possible). It answers `getDocument {depth}`, `querySelector`, `querySelectorAll`, `getOuterHTML`, `setOuterHTML`, `describeNode`, `resolveNode` (-> Runtime object id), `requestNode`, `getBoxModel` (`getBoundingClientRect` -> quads), `focus`, `setAttributeValue`, `removeNode`, `scrollIntoViewIfNeeded`. `DOM.documentUpdated` after navigation.
- Network: `enable`, `requestWillBeSent` and `loadingFinished` from `EngineEvent::Request` and `Load::Complete`, `setCacheDisabled`/`setExtraHTTPHeaders`/`setUserAgentOverride` accepted and stored (applied when Servo exposes them; documented as no-ops). `getResponseBody` -> `-32000 "not available"`.
- Emulation: `setDeviceMetricsOverride` and `clearDeviceMetricsOverride` accepted (no-op), `setEmulatedMedia` -> `{}`; CSS, Debugger, Performance, Overlay, Profiler, HeapProfiler, Security, ServiceWorker, Inspector, Audits `enable`/`disable` -> `{}`.

- [ ] **Step 1: Write failing tests**: `get_document_has_html_root` (FakeEngine eval result fixture), `query_selector_returns_node_id`, `get_box_model_quads`, `request_will_be_sent_from_engine_event`, `stub_enable_returns_empty`.
- [ ] **Step 2: Run** `cargo nextest run -p wren cdp_dom_network`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** tests and the Servo-gated `dom_agent_on_servo` (load a `data:` page with `<p id=x>hi</p>`, `DOM.querySelector('#x')` then `getOuterHTML` equals `<p id="x">hi</p>`).
- [ ] **Step 5: Commit** `feat(cdp): dom, network and stub domains`

### Task 6: Playwright and Puppeteer over CDP

**Files:**
- Create: `parity/cdp/{package.json,bun.lock,playwright.test.ts,puppeteer.test.ts,fixtures/form.html}`, mise task `test:cdp`
- Modify: `docs/automation.md`, site docs `automation.mdx` + `.es.mdx`

**Interfaces:**
- `mise run test:cdp` builds release, starts `wren --remote-debugging-port 9333 --profile-dir <tmp>`, serves `fixtures/` with `Bun.serve` on a random port, runs `bun test parity/cdp`, kills Wren.
- Playwright: `chromium.connectOverCDP("http://127.0.0.1:9333")`. Puppeteer: `puppeteer.connect({ browserURL: "http://127.0.0.1:9333" })`.

- [ ] **Step 1: Write the client tests** (each: new page, `goto` fixture, act, assert):
  - `playwright: title and evaluate` (`page.title() == "Form"`, `page.evaluate(() => 6*7) == 42`).
  - `playwright: fill and click` (`page.fill("#name","wren")`, `page.click("#go")`, `#out` text is `hello wren`).
  - `playwright: screenshot` (buffer starts with PNG magic).
  - `puppeteer: newPage, goto, $eval` (`$eval('#out', e => e.id) == "out"`).
  - `puppeteer: close page` (target count drops by one).
- [ ] **Step 2: Run** `mise run test:cdp`. Expected: FAIL on the first missing method; each failure names it.
- [ ] **Step 3: Implement** the missing methods the clients need, in the Task 4/5 files (expected: `Target.setAutoAttach` details, `Page.getFrameTree`, `Runtime.addBinding` stub, `Emulation.setFocusEmulationEnabled`, `Page.setInterceptFileChooserDialog`, `Network.setRequestInterception` as accepted no-ops). Log every unknown method with `tracing::warn!(target: "cdp", method)`.
- [ ] **Step 4: Run** `mise run test:cdp`. Expected: 5 pass. Write `docs/automation.md` with both snippets.
- [ ] **Step 5: Commit** `feat(cdp): playwright and puppeteer support`

### Task 7: Bundled Chrome DevTools frontend at `wren://devtools`

**Files:**
- Create: `scripts/fetch-devtools-frontend.ts`, `scripts/fetch-devtools-frontend.test.ts`, `assets/devtools-frontend.lock.json`
- Modify: `crates/wren/src/{internal_pages.rs or the wren:// handler},window.rs,actions.rs`, `scripts/bundle.sh`, `NOTICE`, `.gitignore`, docs `devtools.md` + site pages

**Interfaces:**
- `assets/devtools-frontend.lock.json`: `{ "chromium_revision": "<40-hex>", "files": { "<path>": "<sha256>" } }`. The script mirrors `https://chrome-devtools-frontend.appspot.com/serve_rev/@<rev>/` starting from `inspector.html` and `entrypoints/inspector/inspector.js`, following static `import`/`import()` specifiers, `new URL(..., import.meta.url)` assets, CSS `url()` and the `.json` locale files, into `$CARGO_TARGET_DIR/devtools-frontend/<rev>/`; `--verify` checks sha256s against the lock; `--update <rev>` rewrites the lock. Output is never committed.
- `wren://devtools/<path>` serves those files (correct MIME types). The bundle copies them to `Wren.app/Contents/Resources/devtools-frontend/`.
- Action `devtools::Inspect` (`cmd-alt-j`, menu "Inspect with Chrome DevTools"): starts the CDP server if off (port from settings, indicator shows), opens a tab `wren://devtools/inspector.html?ws=127.0.0.1:<port>/devtools/page/<target>` for the active tab.
- Fallback when Servo cannot run the frontend: the same action opens the URL `http://127.0.0.1:<port>/devtools/inspector.html?ws=...` in Chrome through the M1 opener, with the CDP server also serving the files under `/devtools/`. Setting `devtools.frontend = "wren" | "chrome"` (default decided in Step 1).

- [ ] **Step 1: Spike** (throwaway, before tests): fetch the frontend for the current stable revision, load `inspector.html` in Wren against a CDP target, screenshot. Record in the PR whether Elements, Console and Network panels render in Servo 0.8. Set the `devtools.frontend` default to `"wren"` if the Console panel evaluates `1+1`, else `"chrome"`.
- [ ] **Step 2: Write failing tests**: Bun `fetch-devtools-frontend.test.ts` (`extracts static and dynamic imports`, `resolves new URL assets`, `verify fails on hash mismatch`); Rust `wren_devtools_serves_with_mime` (`.js` -> `text/javascript`, `.css` -> `text/css`) and `inspect_starts_server_and_opens_tab` (FakeEngine).
- [ ] **Step 3: Run** `mise run test:repo` and `cargo nextest run -p wren`. Expected: FAIL.
- [ ] **Step 4: Implement**, then run both. Expected: PASS.
- [ ] **Step 5: By hand:** `cmd-alt-j` on `https://example.com`, screenshot (screenshot skill) of the DevTools tab showing Elements with the page's `<h1>`; Console evaluates `document.title`. Put the PNG description in the PR.
- [ ] **Step 6: Commit** `feat: bundled chrome devtools frontend at wren://devtools`

### Task 8: Milestone exit

**Files:**
- Create: `docs/research/perf/2026-11-14-m2.md`
- Modify: roadmap status, `docs/superpowers/plans/2026-11-14-m2-chrome-devtools.md` checkboxes

- [ ] **Step 1:** RAM benchmark: Wren release vs Chrome, tabs10 and tabs20, 3 runs each, CDP off, then tabs10 once with CDP on and Playwright attached. No regression above 10% against `2026-11-14-m1.md`; CDP-on cost reported.
- [ ] **Step 2:** M2 exit: `mise run test:cdp` passes; DevTools frontend inspects a page (Elements, Console); a Puppeteer script and a Playwright script from `docs/automation.md` run against the bundled app.
- [ ] **Step 3: Commit** `chore: m2 exit and measurements`
