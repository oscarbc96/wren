# M3 Extensions Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Manifest V3 Chrome extensions install from the Chrome Web Store
or an unpacked folder and run in Wren: content scripts and CSS, a
background context, the action popup and options page, messaging, and
the core `chrome.*` APIs, measured by an extension harness that covers
uBlock Origin Lite, Dark Reader, Bitwarden and 1Password.

**Architecture:** a new `extensions` crate owns everything that does not
touch Servo: manifest parsing, CRX3 download and verification, the
installed-extension registry, match patterns, permissions, and the
`chrome.*` API implementation as a pure Rust `ApiHost` that answers JSON
calls. The `engine` crate gains three Servo 0.8 hooks: a custom scheme
(`ProtocolRegistry`) that serves `chrome-extension://<id>/...` files and
an RPC endpoint, user content (`UserContentManager`: scripts and
stylesheets per webview), and hidden webviews. Page-side JavaScript
(`chrome_api.js`) builds the `chrome` object and calls Rust with
`fetch("chrome-extension://<id>/__wren/api", {method:"POST"})`; Rust
pushes events back with `Engine::evaluate`. Servo has no isolated
worlds, so content scripts run in the page's world inside a closure
whose bridge is captured before page scripts run (`document_start`);
this is weaker isolation than Chrome and is documented as such until a
Servo patch adds worlds.

**Tech Stack:** as M2, plus `prost` (CRX3 header protobuf), `rsa` +
`sha2` + `p256` (CRX3 signatures), `zip`, `serde_json`, `globset`-free
own match-pattern code.

**Spec:** `docs/superpowers/specs/2026-11-13-wren-browser-design.md`
(sections 5b licence table, 7.3 harness item 3, 7.4, 9, 10 M3, 11).
Roadmap: `docs/superpowers/plans/2026-11-13-roadmap.md`.

## Global Constraints

- All M0 to M2 constraints hold (MPL-2.0, only `engine` imports `servo`,
  macOS 14+ Apple Silicon, signed squash PRs, local green).
- Manifest V3 only. MV2 extensions are refused with a clear message.
- Extensions download only on the user's request from
  `https://clients2.google.com/service/update2/crx?response=redirect&acceptformat=crx3&prodversion=<extensions.chrome_version>&x=id%3D<id>%26uc`;
  Wren never redistributes them. Setting `extensions.chrome_version`
  default `"142.0.0.0"`.
- A CRX whose signature does not verify against its embedded key and the
  Web Store publisher key is never installed.
- Install shows the permission list and installs only on confirm.
  Extensions are stored under `<profile>/extensions/<id>/<version>/`.
- API order (spec 7.4): `runtime`, `storage`, `tabs`, `scripting`,
  `i18n`, `action`, `contextMenus`, `alarms`, `notifications`,
  `declarativeNetRequest` (backed by `adblock_rules`), then
  `webNavigation`, `cookies`, `identity`, `sidePanel` as the harness
  demands. Unimplemented API members exist and reject with
  `"Wren does not support chrome.<ns>.<fn> yet"` so feature detection
  works.
- RAM: background contexts are stopped after 30 s idle (Chrome's MV3
  service worker rule) and restarted on the next event; popups close
  their webview on blur. Each running background counts toward the RAM
  benchmark.
- The `chrome-extension://__wren/api` endpoint answers only requests
  from that extension's own origin or its content scripts' webviews;
  any other caller gets 403.

## Review Focus

1. **A web page calling `fetch("chrome-extension://<id>/__wren/api")`**
   must be refused, so pages cannot use an extension's privileges.
   Pinned by `page_origin_cannot_call_api` (Task 3).
2. **A CRX with a tampered payload or a missing signature** must not
   install and must leave nothing on disk. Pinned by
   `tampered_crx_rejected_and_cleaned` (Task 1).
3. **`<all_urls>` and `*://*/*` content scripts** must not run on
   `wren://` pages or `chrome-extension://` pages of other extensions.
   Pinned by `content_scripts_skip_internal_pages` (Task 2).
4. **A background that is idle-stopped while a popup holds a port** must
   restart on the next message and the port must report disconnect, not
   hang. Pinned by `idle_stop_disconnects_ports` (Task 4).
5. **`storage.local` writes from two contexts at once** must both land
   (last write per key wins, no lost keys). Pinned by
   `concurrent_storage_writes_keep_all_keys` (Task 3).

---

### Task 1: `extensions` crate: manifest, CRX3, registry

**Files:**
- Create: `crates/extensions/{Cargo.toml,src/lib.rs,src/manifest.rs,src/crx.rs,src/registry.rs,src/match_pattern.rs,src/permissions.rs}`
- Create: `crates/extensions/tests/{manifest.rs,crx.rs,match_pattern.rs,registry.rs}`, fixtures under `crates/extensions/tests/fixtures/`
- Modify: `crates/store` (migration `003_extensions.sql`), workspace `Cargo.toml`

**Interfaces:**
- Produces:
  - `pub struct ExtensionId(pub String)` (32 chars `a`..`p`, derived from the public key: first 16 bytes of SHA-256, each nibble + `'a'`).
  - `pub struct Manifest { name, version, manifest_version: u8, description, default_locale, permissions: Vec<String>, host_permissions: Vec<String>, content_scripts: Vec<ContentScript>, background: Option<Background>, action: Option<Action>, options_page: Option<String>, options_ui: Option<OptionsUi>, icons: BTreeMap<u32,String>, web_accessible_resources: Vec<WebAccessible>, declarative_net_request: Option<Dnr>, key: Option<String> }` with `Manifest::parse(json: &str, locale: Option<&Messages>) -> Result<Manifest, ManifestError>` (`__MSG_name__` substitution; MV2 -> `ManifestError::Mv2`).
  - `pub struct ContentScript { matches, exclude_matches, js, css, run_at: RunAt, all_frames: bool, world: World }`, `RunAt::{DocumentStart, DocumentEnd, DocumentIdle}`, `World::{Isolated, Main}`.
  - `pub struct MatchPattern; MatchPattern::parse(&str) -> Result<Self, PatternError>; fn matches(&self, url: &Url) -> bool` (Chrome semantics: `<all_urls>`, scheme `*` = http/https, `*.host`, path glob; never matches `wren:` or `chrome-extension:`).
  - `crx::verify_and_unpack(bytes: &[u8], dest: &Path) -> Result<(ExtensionId, Manifest), CrxError>` (CRX3: magic `Cr24`, version 3, `CrxFileHeader` protobuf, `SignedData.crx_id` must equal the id from the key, at least one RSA-PKCS1-SHA256 or ECDSA-P256-SHA256 proof over `"CRX3 SignedData\x00" + len(signed_header_data) as u32 LE + signed_header_data + zip`; unpack to a temp dir, rename into place only on success).
  - `pub async fn download_from_web_store(id: &ExtensionId, chrome_version: &str) -> Result<Vec<u8>, CrxError>` (reqwest, rustls).
  - `Registry` over `store`: `install(id, manifest, path)`, `list()`, `get(id)`, `set_enabled(id, bool)`, `remove(id)` (deletes files), `installed_version(id)`.
  - `permissions::summarize(&Manifest) -> Vec<String>` (human lines for the install prompt, e.g. "Read and change all your data on all websites").

- [ ] **Step 1: Write failing tests**: `parses_mv3_manifest_with_messages`, `mv2_refused`, `id_from_key_is_32_a_to_p`, `match_pattern_chrome_cases` (table: `<all_urls>` matches `https://a.com/` and `file:///x`, not `wren://settings`; `*://*.google.com/*` matches `https://mail.google.com/x`, not `https://google.com.evil.io/`; `https://a.com/foo*` matches `/foobar` not `/bar`), `valid_crx_installs` (fixture CRX built in the test from a generated RSA key with the same header layout), `tampered_crx_rejected_and_cleaned` (flip one zip byte: `CrxError::BadSignature`, dest dir absent), `registry_roundtrip`.
- [ ] **Step 2: Run** `cargo nextest run -p extensions`. Expected: FAIL.
- [ ] **Step 3: Implement.** The `CrxFileHeader` message is three fields (`sha256_with_rsa = 2`, `sha256_with_ecdsa = 3`, `signed_header_data = 10000`); write the `prost` structs by hand, no build script.
- [ ] **Step 4: Run** the tests; `cargo deny check`.
- [ ] **Step 5: Commit** `feat(extensions): manifest, crx3 verification and registry`

### Task 2: Engine hooks: extension scheme, user content, hidden webviews

**Files:**
- Modify: `crates/engine/src/{api.rs,fake.rs,servo_engine.rs}`
- Test: `crates/engine/tests/fake_extension_hooks.rs`

**Interfaces:**
- Produces on `Engine`:
  - `fn register_scheme(&mut self, scheme: &str, handler: Arc<dyn SchemeHandler>)` before the first webview; `pub trait SchemeHandler: Send + Sync { fn load(&self, req: SchemeRequest) -> SchemeResponse; }` with `SchemeRequest { url: Url, method: String, body: Vec<u8>, origin: Option<String>, webview: Option<WebViewId> }`, `SchemeResponse { status: u16, content_type: String, body: Vec<u8> }`. Servo: `ServoBuilder::protocol_registry` with a `ProtocolHandler` adapter (`is_fetchable() == true`, `is_secure() == true`).
  - `fn set_user_content(&mut self, id: WebViewId, content: UserContent)` where `UserContent { scripts: Vec<UserScriptSpec { source: String, url: Url, run_at: RunAt, all_frames: bool }>, styles: Vec<(String, Url)> }`. Servo: one `UserContentManager` per webview set at creation; changing it rebuilds on next navigation. `document_end`/`document_idle` scripts wrap themselves in a `DOMContentLoaded` / `load` listener when injected at start.
  - `fn create_hidden(&mut self, url: &Url) -> WebViewId` (never shown, not in the tab strip, still counted by `processes()` with `kind: "extension"`).
- `content_scripts_skip_internal_pages` lives here as the engine-side filter: the engine never applies user content to `wren:` or other `chrome-extension:` origins.

- [ ] **Step 1: Write failing tests** with `FakeEngine`: `scheme_handler_receives_post_body`, `user_content_recorded_per_webview`, `hidden_webview_not_listed_as_tab_process`, `content_scripts_skip_internal_pages`.
- [ ] **Step 2: Run** `cargo nextest run -p engine`. Expected: FAIL.
- [ ] **Step 3: Implement** Fake and Servo. Servo: read `servo-0.8.0/user_content_manager.rs` and `servo-net-0.8.0/protocols/mod.rs` first.
- [ ] **Step 4: Run** tests; by hand with the `servo_in_gpui` example: register `wrentest:` returning `text/html` "ok", load it, page shows "ok"; a user script setting `document.title = "injected"` changes the title event.
- [ ] **Step 5: Commit** `feat(engine): extension scheme, user content and hidden webviews`

### Task 3: Runtime glue: `chrome_api.js`, RPC endpoint, `runtime` + `storage`

**Files:**
- Create: `crates/extensions/src/{api_host.rs,api/runtime.rs,api/storage.rs,js/chrome_api.js,js/content_prelude.js}`
- Create: `crates/wren/src/extensions_host.rs` (wires `ApiHost` to `Engine`, tabs and store)
- Test: `crates/extensions/tests/api_host.rs`, `crates/wren/tests/extensions_host.rs`

**Interfaces:**
- `pub struct ApiHost` with `fn call(&mut self, ctx: CallContext, ns: &str, method: &str, args: Value) -> ApiResult` where `CallContext { extension: ExtensionId, kind: ContextKind::{Background, Popup, Options, ContentScript { tab: TabId, frame: u64 }}, }` and `ApiResult::{Ok(Value), Err(String), Pending(PendingId)}` (pending answers later via `ApiHost::resolve`). Events out: `ApiHost::drain_events() -> Vec<(Target, String /*event name*/, Value)>`.
- RPC wire: `POST chrome-extension://<id>/__wren/api` body `{"ns","method","args","ctx"}` -> `{"ok":value}` or `{"err":"..."}`. The scheme handler checks `origin == chrome-extension://<id>` or that `webview` has that extension's content script injected; else 403.
- `chrome_api.js` builds `chrome` (and `browser` alias) with promise and callback forms, `chrome.runtime.lastError`, `chrome.runtime.{id, getManifest, getURL, sendMessage, onMessage, connect, onConnect, onInstalled, onStartup, getPlatformInfo, openOptionsPage}`, `chrome.storage.{local, sync, session}.{get,set,remove,clear,getBytesInUse}` plus `onChanged`. `sync` is local-only (no account sync), documented.
- Events reach JS through `Engine::evaluate(webview, "window.__wrenExt.dispatch(<json>)")`.
- Storage persists in `store` table `extension_storage(extension_id, area, key, value_json)`; writes are per key so concurrent writers never drop keys.

- [ ] **Step 1: Write failing tests**: `runtime_get_url`, `send_message_reaches_background_and_replies`, `on_installed_fires_once`, `storage_set_get_remove`, `storage_on_changed_event`, `concurrent_storage_writes_keep_all_keys`, `page_origin_cannot_call_api`, `unsupported_member_rejects_with_message`.
- [ ] **Step 2: Run** `cargo nextest run -p extensions -p wren`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** tests; by hand: unpacked fixture extension `crates/extensions/tests/fixtures/hello/` (content script sends a message, background replies, content script writes the reply into `document.body.dataset.wren`); load `https://example.com`, check with `scripts/cdp-eval.ts` that `document.body.dataset.wren == "pong"`.
- [ ] **Step 5: Commit** `feat(extensions): chrome api bridge, runtime and storage`

### Task 4: Background contexts and lifecycle

**Files:**
- Create: `crates/extensions/src/background.rs`
- Modify: `crates/wren/src/extensions_host.rs`
- Test: `crates/wren/tests/extension_background.rs`

**Interfaces:**
- MV3 `background.service_worker` runs as an event page: a hidden webview at `chrome-extension://<id>/__wren/background.html` whose HTML loads `chrome_api.js` then the worker script (`type="module"` when `background.type == "module"`), with `self`, `importScripts` (sync XHR-free: concatenated at serve time for classic scripts) and `self.registration` shims. Real Servo service workers replace this when upstream support is ready (tracked in the Servo patch queue).
- Lifecycle: start on install, browser start (if it has listeners for `onStartup`), and on any event addressed to it; stop after `extensions.background_idle_seconds` (default 30) with no pending calls, open ports or alarms due within that window. `idle_stop_disconnects_ports` closes ports with `onDisconnect`.
- Task manager rows show each running background as "Extension: <name>" with End process.

- [ ] **Step 1: Write failing tests**: `background_starts_on_install`, `background_stops_after_idle`, `event_restarts_stopped_background`, `idle_stop_disconnects_ports`, `background_listed_in_task_manager`.
- [ ] **Step 2: Run** `cargo nextest run -p wren --test extension_background`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** tests; RAM check by hand: `hello` fixture installed, background stops after 30 s (process count drops by one in `parity ram`).
- [ ] **Step 5: Commit** `feat(extensions): background contexts with idle stop`

### Task 5: `tabs`, `scripting`, `i18n`, `alarms`, `notifications`, `contextMenus`

**Files:**
- Create: `crates/extensions/src/api/{tabs.rs,scripting.rs,i18n.rs,alarms.rs,notifications.rs,context_menus.rs}`
- Modify: `crates/wren/src/{extensions_host.rs,window.rs}` (context menu items, notification banners)
- Test: `crates/extensions/tests/api_more.rs`, `crates/wren/tests/extension_apis.rs`

**Interfaces:**
- `tabs.{query, get, create, update, remove, sendMessage, onUpdated, onActivated, onRemoved, onCreated, captureVisibleTab}` mapped onto `TabStrip` and `Engine::screenshot`; `Tab` objects carry `url`/`title` only with the `tabs` permission or host access.
- `scripting.{executeScript, insertCSS, removeCSS}` via `Engine::evaluate` (files or `func` + `args`), host permission checked.
- `i18n.{getMessage, getUILanguage, detectLanguage}` (detectLanguage returns `"und"`), `_locales/<lang>/messages.json` with placeholders.
- `alarms` persisted in `store`, fire through the background lifecycle (min period 30 s, as Chrome).
- `notifications.create/clear` show an in-window banner (no macOS notification centre in M3).
- `contextMenus.create/update/remove/removeAll` add items to the page context menu; `onClicked` reaches the background.

- [ ] **Step 1: Write failing tests**: `tabs_query_active`, `tabs_hide_url_without_permission`, `execute_script_func_with_args`, `insert_css_needs_host_permission`, `i18n_placeholders`, `alarm_fires_and_wakes_background`, `context_menu_click_reaches_background`.
- [ ] **Step 2: Run** `cargo nextest run -p extensions -p wren`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** tests; screenshot skill with the `hello` fixture registering a context menu item: right-click on a page, PNG shows the item.
- [ ] **Step 5: Commit** `feat(extensions): tabs, scripting, i18n, alarms, notifications, context menus`

### Task 6: `action`, popup, options and the extensions page

**Files:**
- Create: `crates/extensions/src/api/action.rs`, `crates/wren/src/extensions_ui.rs`, `crates/wren/src/internal/extensions.html` (or the existing `wren://` page mechanism)
- Modify: `crates/wren/src/{window.rs,layout.rs,actions.rs}`
- Test: `crates/wren/tests/extensions_ui.rs`

**Interfaces:**
- Toolbar: one icon button per enabled extension with an `action` (pinned ones visible, others under a puzzle menu); badge text and colour from `action.setBadgeText/setBadgeBackgroundColor`; `setIcon`, `setTitle`, `setPopup`, `onClicked` when no popup.
- Popup: a webview of `chrome-extension://<id>/<default_popup>` in a GPUI overlay anchored under the button, sized to its content (max 800x600), closed on blur or Escape (webview closed, RAM freed).
- Options: `options_ui.page` opens in a tab (`open_in_tab` honoured either way in M3).
- `wren://extensions`: list (icon, name, version, enabled toggle, remove, details with permissions), "Install from Chrome Web Store" field accepting a Web Store URL or id, "Load unpacked" folder picker. Install flow: download, verify, permission prompt (sheet listing `permissions::summarize`), confirm, `onInstalled`.
- Actions: `extensions::Open` (menu), `extensions::Reload` (developer mode, unpacked only).

- [ ] **Step 1: Write failing tests**: `action_button_shows_badge`, `popup_opens_and_closes_on_blur`, `install_from_store_url_parses_id` (`https://chromewebstore.google.com/detail/dark-reader/eimadpbcbfnmbkopoojfekhnkhdbieeh` -> id), `install_requires_confirm`, `disable_removes_content_scripts`.
- [ ] **Step 2: Run** `cargo nextest run -p wren --test extensions_ui`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** tests; screenshot skill: `wren://extensions` with the `hello` fixture loaded unpacked, and the popup open over a page. Describe both ONGs in the PR.
- [ ] **Step 5: Commit** `feat(extensions): action, popup, options and extensions page`

### Task 7: `declarativeNetRequest` on the adblock engine

**Files:**
- Create: `crates/extensions/src/api/dnr.rs`
- Modify: `crates/adblock_rules` (accept DNR rules), `crates/wren/src/extensions_host.rs`
- Test: `crates/extensions/tests/dnr.rs`

**Interfaces:**
- Static rulesets from `declarative_net_request.rule_resources` (enabled ones), dynamic and session rules via `updateDynamicRules`/`updateSessionRules`/`getDynamicRules`/`getEnabledRulesets`/`updateEnabledRulesets`.
- Supported actions: `block`, `allow`, `allowAllRequests`, `upgradeScheme`; `redirect` to an extension path or URL; `modifyHeaders` accepted and stored, applied when Servo exposes header rewriting (documented). Conditions: `urlFilter`, `regexFilter`, `requestDomains`, `excludedRequestDomains`, `initiatorDomains`, `resourceTypes`, `excludedResourceTypes`, `domainType`.
- Rules feed the same request filter as M1 adblock; documents are still never blocked by cosmetic lists, but DNR `block` on `main_frame` is honoured (Chrome semantics).

- [ ] **Step 1: Write failing tests**: `dnr_block_by_url_filter`, `dnr_allow_overrides_block`, `dnr_resource_type_filter`, `dnr_redirect_to_extension_path`, `dynamic_rules_persist`.
- [ ] **Step 2: Run** `cargo nextest run -p extensions -p adblock_rules`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** tests.
- [ ] **Step 5: Commit** `feat(extensions): declarativeNetRequest on the adblock engine`

### Task 8: Extension harness and the four targets

**Files:**
- Create: `crates/parity/src/extensions.rs`, `data/top-extensions.txt` (top 100 ids with names), `crates/parity/smoke/extensions/<id>.js` for the four targets
- Modify: `crates/parity/src/main.rs` (subcommand `parity extensions`), mise task `parity:extensions`, `docs/research/perf/`
- Test: `crates/parity/tests/extensions.rs`

**Interfaces:**
- `parity extensions [--only <id>] [--limit N] --out <json>`: for each id, download (cached under the target dir, never committed), install into a throwaway profile, open a fixture page served locally, run its smoke script over CDP (M2), record `install_ok`, `background_ok`, `content_ok`, `popup_ok`, `missing_apis` (from the `unsupported member` rejections, collected through console events), pass/fail.
- Smoke scripts for: uBlock Origin Lite (`ddkjiahejlhfcafbddmgiahcphecmpfh`: a known ad URL on the fixture page is blocked), Dark Reader (`eimadpbcbfnmbkopoojfekhnkhdbieeh`: fixture page background turns dark), Bitwarden (`nngceckbapebfimnlniiiahkandclblb`: popup renders its login screen), 1Password (`aeblfdkhhhdcdjpifhhbdiojplfjncoa`: popup renders; native messaging is expected to be missing and is reported, not a crash).

- [ ] **Step 1: Write failing tests**: `report_counts_pass_and_missing_apis` (pure, fixture results), `smoke_script_lookup_by_id`.
- [ ] **Step 2: Run** `cargo nextest run -p parity`. Expected: FAIL.
- [ ] **Step 3: Implement**; run `mise run parity:extensions -- --limit 100`, commit the JSON and a summary `docs/research/perf/2026-xx-xx-m3-extensions.md` (pass count, the four targets, top missing APIs).
- [ ] **Step 4: Fix** whatever the four targets need within the API order (each fix with a test in the owning task's test file).
- [ ] **Step 5: Commit** `feat(parity): extension harness and smoke tests`

### Task 9: Milestone exit

- [ ] **Step 1:** RAM benchmark tabs10/tabs20 vs Chrome, clean and with uBO Lite + Dark Reader installed in both; no regression above 10% against M2 with extensions off; extension cost reported.
- [ ] **Step 2:** M3 exit: the four targets install from the Web Store and pass their smoke tests (1Password: popup renders, native messaging gap reported); harness pass count recorded.
- [ ] **Step 3:** Roadmap status; **Commit** `chore: m3 exit and measurements`
