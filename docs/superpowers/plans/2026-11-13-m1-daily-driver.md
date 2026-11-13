# M1 Daily Driver Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** the owner can use Wren as the everyday browser: tabs that can be
killed one by one and sleep when idle, blocked ads, saved passwords from
the Keychain and Chrome, downloads, per-site "Open in Chrome", Servo
DevTools, and a top-sites harness whose must-work list (Google Calendar,
Google Meet, Slack, DevTools) passes or is routed to Chrome.

**Architecture:** new behaviour sits behind the `Engine` trait so `wren`
keeps testing against `FakeEngine`. Servo hooks used (all in Servo 0.8):
`WebViewDelegate::load_web_resource` (request blocking),
`request_navigation` (Open in Chrome rules, downloads),
`WebView::evaluate_javascript` (hang ping, password fill, harness probes),
`take_screenshot` and `show_console_message` (harness),
`notify_media_session_event` (do not sleep playing tabs) and the
`devtools_server_enabled` preference. New crates: `adblock_rules`
(filter lists, wraps Brave's `adblock`), `creds` (Keychain store, Chrome
and CSV import). Everything else extends `engine`, `tabs`, `store`,
`settings`, `wren` and `parity`.

**Tech Stack:** as M0, plus `adblock` (Brave, MPL-2.0),
`security-framework` (Keychain), `aes`/`cbc`/`pbkdf2`/`sha1` (Chrome
password decryption), `csv`, `reqwest` with rustls (filter list and
download fetches), `image` + `image-compare` (harness screenshots).

**Spec:** `docs/superpowers/specs/2026-11-13-wren-browser-design.md`
(sections 7.2, 7.3, 7.5, 7.6, 9, 10 M1, 11, 12).
Roadmap: `docs/superpowers/plans/2026-11-13-roadmap.md`.

## Global Constraints

- All M0 constraints hold (license, crate rules, only `engine` imports
  `servo`, macOS 14+ Apple Silicon, signed squash PRs, local green).
- One content process per tab (spec 7.2). Never fall back to a single
  process; the RAM fallback is one process per site.
- Network calls beyond page loads are limited to: filter list updates
  (weekly, setting `adblock.update_lists`, default `true`) and downloads
  the user starts. No telemetry.
- Passwords never leave the Keychain in plain text: not in SQLite, logs,
  crash reports or test fixtures. Tests use a test-only Keychain service
  name and delete their items.
- Settings defaults: `tabs.sleep_after_minutes = 30`,
  `tabs.memory_budget_mb = 2048`, `hang.timeout_seconds = 5`,
  `adblock.enabled = true`, `sites.open_in_chrome = []`,
  `devtools.port = 7000`, `downloads.dir` = `~/Downloads`.
- Builds stay ad-hoc signed (`codesign -s -`); no Apple account needed.

## Review Focus

1. **Killing the active tab's process** must show its crash notice and
   keep every other tab and the window usable. Pinned by
   `end_process_marks_only_that_tab` (Task 1).
2. **A slept tab that is clicked** must come back at the same URL, and a
   tab playing media, pinned, or with unsaved form input must never
   sleep. Pinned by `sleep_skips_media_and_pinned` and
   `woken_tab_restores_url` (Task 2).
3. **A blocking rule that matches the page itself** (first-party
   document) must not blank the page: documents are never blocked, only
   subresources. Pinned by `document_requests_never_blocked` (Task 3).
4. **Chrome import with a locked or missing Keychain entry** must fail
   with a clear message and import nothing, never partial garbage.
   Pinned by `chrome_import_without_key_errors` (Task 4).
5. **An Open-in-Chrome rule for a host** must also catch redirects into
   that host (e.g. accounts.google.com to meet.google.com). Pinned by
   `redirect_into_rule_host_opens_chrome` (Task 6).

---

### Task 1: Task manager, End process and the hang watchdog

**Files:**
- Modify: `crates/engine/src/{api.rs,fake.rs,servo_engine.rs}`
- Modify: `crates/tabs/src/lib.rs`
- Create: `crates/wren/src/task_manager.rs`
- Test: `crates/engine/tests/hang.rs`, `crates/wren/tests/task_manager.rs`

**Interfaces:**
- Produces in `engine::api`: `EngineEvent::Hung { id }`,
  `EngineEvent::Responsive { id }`; `Engine::kill(&mut self, id: WebViewId)`
  (default no-op); `Engine::processes(&mut self) -> Vec<ProcessInfo>` with
  `pub struct ProcessInfo { pub id: Option<WebViewId>, pub pid: u32, pub rss_bytes: u64, pub cpu_percent: f32 }`
  (`id: None` for the browser process itself).
- Produces in `tabs`: `Tab.hung: bool`, set by `Hung`, cleared by
  `Responsive`, `Load`, `Crashed`.
- GPUI action `task_manager::Toggle` on `cmd-alt-escape`.

- [ ] **Step 1: Write failing tests**: `hang_after_timeout` (FakeEngine
  pushes `Hung`; the tab shows `hung`), `responsive_clears_hung`,
  `end_process_marks_only_that_tab` (FakeEngine `kill` then `Crashed`
  for that id; other tabs untouched), `task_manager_lists_tabs` (rows
  for the browser and each tab with title, RSS, CPU).
- [ ] **Step 2: Run** `cargo nextest run -p engine -p tabs -p wren`.
  Expected: FAIL.
- [ ] **Step 3: Implement.** ServoEngine pings each live webview every
  second with `evaluate_javascript("0")`; no reply within
  `hang.timeout_seconds` gives `Hung`, the next reply `Responsive`.
  `kill` sends SIGKILL to the tab's pid from the child-process map (the
  watcher then emits `Crashed`). `processes` uses `sysinfo` for RSS and
  CPU. `task_manager.rs` is a GPUI overlay listing rows sorted by RSS
  with an End process button per tab row; a hung tab shows a bar in the
  content area: "This page is not responding" with Wait and End process.
- [ ] **Step 4: Run** tests, then by hand: open a page running
  `while(true){}` (data URL), see the bar within 6 s, End process, the
  crash notice appears, other tabs keep working.
- [ ] **Step 5: Commit** `feat: task manager, end process and hang watchdog`

### Task 2: Tab sleep and memory budget

**Files:**
- Modify: `crates/tabs/src/lib.rs`, `crates/engine/src/{api.rs,servo_engine.rs}`, `crates/wren/src/window.rs`
- Test: `crates/tabs/tests/sleep.rs`

**Interfaces:**
- Produces in `tabs`: `pub enum TabState { Active, Background, Slept }`,
  `Tab.state`, `Tab.pinned: bool`, `Tab.audible: bool`,
  `Tab.last_active: jiff::Timestamp`,
  `TabStrip::sleep(&mut self, e: &mut dyn Engine, id: TabId)`,
  `TabStrip::wake(&mut self, e: &mut dyn Engine, id: TabId)`,
  `TabStrip::sleep_candidates(&self, now: jiff::Timestamp, after: jiff::SignedDuration) -> Vec<TabId>`
  (oldest first; never active, pinned or audible).
- Produces in `engine`: `EngineEvent::Audible { id, playing: bool }` from
  `notify_media_session_event`.

- [ ] **Step 1: Write failing tests**: `sleep_closes_webview_keeps_url`,
  `woken_tab_restores_url`, `sleep_skips_media_and_pinned`,
  `candidates_oldest_first`, `budget_sleeps_until_under`
  (given RSS per tab, sleep candidates until the sum is under the
  budget).
- [ ] **Step 2: Run** `cargo nextest run -p tabs`. Expected: FAIL.
- [ ] **Step 3: Implement.** Sleeping closes the webview (its content
  process exits) and keeps URL and title; activating a slept tab creates
  a new webview at that URL. The window checks every 60 s: tabs idle
  longer than `tabs.sleep_after_minutes` sleep; if the process tree RSS
  is above `tabs.memory_budget_mb`, sleep candidates in order until under.
  Slept tabs render dimmed in the strip. Tab context menu: Pin, Sleep.
- [ ] **Step 4: Run** tests; by hand open 10 tabs, set the budget to
  600 MB, see background tabs sleep and wake on click.
- [ ] **Step 5: Commit** `feat(tabs): sleep idle tabs and keep a memory budget`

### Task 3: Ad and tracker blocking (`adblock_rules`)

**Files:**
- Create: `crates/adblock_rules/{Cargo.toml,src/lib.rs}`
- Modify: `crates/engine/src/{api.rs,servo_engine.rs}`, `crates/wren/src/app.rs`
- Test: `crates/adblock_rules/tests/rules.rs`

**Interfaces:**
- Produces: `pub struct Blocker`, `Blocker::from_lists(lists: &[&str]) -> Blocker`,
  `Blocker::should_block(&self, url: &Url, source: &Url, kind: RequestKind) -> bool`,
  `pub enum RequestKind { Document, Script, Image, Stylesheet, Xhr, Font, Media, Other }`,
  `pub fn update_lists(dir: &Path) -> anyhow::Result<Vec<PathBuf>>`
  (downloads EasyList and EasyPrivacy into the data dir, at most weekly).
- Engine: `Engine::set_request_filter(&mut self, f: Box<dyn Fn(&Url, &Url, RequestKind) -> bool>)`
  applied in `load_web_resource` (blocked requests get an empty response).

- [ ] **Step 1: Write failing tests** with an inline list:
  `blocks_known_tracker` (`||doubleclick.net^` blocks a script from a
  page on example.org), `allows_exception_rule` (`@@` wins),
  `document_requests_never_blocked`, `third_party_option_respected`.
- [ ] **Step 2: Run** `cargo nextest run -p adblock_rules`. Expected: FAIL.
- [ ] **Step 3: Implement** with the `adblock` crate's `Engine`. The app
  loads cached lists at start (no network on the startup path), updates
  them in the background, and shows a per-tab blocked count in the
  toolbar. Setting `adblock.enabled` toggles the filter.
- [ ] **Step 4: Run** tests; by hand load a news site with and without
  blocking and record RSS and blocked counts in the PR body.
- [ ] **Step 5: Commit** `feat(adblock_rules): block ads and trackers`

### Task 4: Passwords (`creds`): Keychain store, Chrome and CSV import, fill

**Files:**
- Create: `crates/creds/{Cargo.toml,src/lib.rs,src/keychain.rs,src/chrome.rs,src/csv.rs}`
- Modify: `crates/engine/src/api.rs` (`Engine::run_script`), `crates/wren/src/window.rs`
- Test: `crates/creds/tests/{keychain.rs,chrome.rs,csv.rs}`

**Interfaces:**
- Produces: `pub struct Credential { pub origin: String, pub username: String, pub password: String }`,
  `pub trait CredStore { fn save(&self, c: &Credential) -> Result<()>; fn find(&self, origin: &str) -> Result<Vec<Credential>>; fn delete(&self, origin: &str, username: &str) -> Result<()>; fn list_origins(&self) -> Result<Vec<String>>; }`,
  `KeychainStore::new(service: &str)` (service `wren.passwords`; tests use
  `wren.passwords.test`), `MemoryCredStore`,
  `chrome::import(profile_dir: &Path, store: &dyn CredStore) -> Result<usize>`,
  `csv::import(path: &Path, store: &dyn CredStore) -> Result<usize>`.
- Engine: `Engine::run_script(&mut self, id: WebViewId, js: &str)`.
- GPUI action `passwords::Fill` on `cmd-\`.

- [ ] **Step 1: Write failing tests**: `keychain_roundtrip` (save, find,
  delete under the test service), `csv_import_chrome_format` (Chrome's
  `name,url,username,password` header), `chrome_decrypts_v10` (fixture
  `Login Data` built in the test with a known key and AES-128-CBC
  `v10` blob), `chrome_import_without_key_errors`, `fill_script_escapes`
  (the generated fill script JSON-escapes username and password).
- [ ] **Step 2: Run** `cargo nextest run -p creds`. Expected: FAIL.
- [ ] **Step 3: Implement.** Chrome import reads the `Chrome Safe
  Storage` password from the Keychain (the system prompts the user),
  derives the key with PBKDF2-SHA1 (salt `saltysalt`, 1003 rounds, 16
  bytes), decrypts `v10` values, and copies `Login Data` to a temp file
  before opening it. `passwords::Fill` finds credentials for the active
  origin (one: fill; several: a GPUI picker) and runs a script that sets
  the first password field and the username field before it, firing
  `input` events. Settings page entry: Import from Chrome, Import CSV.
- [ ] **Step 4: Run** tests; by hand import from the owner's Chrome and
  fill a login form.
- [ ] **Step 5: Commit** `feat(creds): keychain passwords, chrome and csv import, fill`

### Task 5: Downloads

**Files:**
- Create: `crates/wren/src/downloads.rs`
- Modify: `crates/engine/src/{api.rs,servo_engine.rs}`, `crates/store/src/lib.rs` (downloads table)
- Test: `crates/wren/tests/downloads.rs`, `crates/store/tests/downloads.rs`

**Interfaces:**
- Engine: `EngineEvent::Download { id: WebViewId, url: Url, suggested_name: Option<String> }`
  emitted when a navigation targets a URL whose response Servo cannot
  render (research step below) or that has a download extension.
- Store: `record_download`, `list_downloads`, `update_download` with
  `pub struct DownloadRow { pub id: i64, pub url: String, pub path: String, pub bytes: u64, pub total: Option<u64>, pub state: DownloadState }`.

- [ ] **Step 1: Research (30 min cap)**: how Servo 0.8 handles a
  `Content-Disposition: attachment` response (search servo-net and
  servo-script for "attachment"). Record in
  `docs/research/2026-11-xx-servo-downloads.md`. If Servo exposes no
  hook, use `request_navigation` + a HEAD request to detect attachments
  and file types (`.zip .dmg .pkg .pdf .csv .xlsx .docx .mp4 .tar.gz`).
- [ ] **Step 2: Write failing tests**: `download_event_saves_file`
  (local test server, file lands in `downloads.dir` with the suggested
  name, no overwrite: `name (1).ext`), `downloads_table_roundtrip`,
  `cancel_removes_partial`.
- [ ] **Step 3: Implement.** Fetch with `reqwest` (cookies for the
  origin are not shared yet: note it in the research doc; the Servo
  patch queue fixes it if authenticated downloads fail). Toolbar shows
  a downloads button with progress; a panel lists recent downloads with
  Open and Show in Finder.
- [ ] **Step 4: Run** tests; by hand download a public `.zip` and `.pdf`.
- [ ] **Step 5: Commit** `feat: downloads`

### Task 6: Open in Chrome and per-site rules

**Files:**
- Create: `crates/wren/src/open_in_chrome.rs`
- Modify: `crates/engine/src/{api.rs,servo_engine.rs}`, `crates/settings` defaults
- Test: `crates/wren/tests/open_in_chrome.rs`

**Interfaces:**
- Engine: `Engine::set_navigation_filter(&mut self, f: Box<dyn Fn(&Url) -> bool>)`
  (false denies the navigation and emits `EngineEvent::NavigationDenied { id, url }`).
- `pub fn rule_matches(rules: &[String], url: &Url) -> bool` (host or
  subdomain match, `*.example.com` allowed).
- GPUI action `site::OpenInChrome` (`cmd-shift-o`) and a toolbar menu
  item "Always open this site in Chrome" that appends the host to
  `sites.open_in_chrome`.

- [ ] **Step 1: Write failing tests**: `rule_matches_subdomains`,
  `redirect_into_rule_host_opens_chrome` (FakeEngine emits
  `NavigationDenied` for a redirect target; the app calls the opener
  with that URL and keeps the tab on its previous page),
  `open_in_chrome_action_uses_active_url`.
- [ ] **Step 2: Run** `cargo nextest run -p wren`. Expected: FAIL.
- [ ] **Step 3: Implement** with `open -a "Google Chrome" <url>` behind a
  `trait Opener` (tests use a recording fake). If Chrome is missing,
  fall back to the default browser and show a notice.
- [ ] **Step 4: Run** tests; by hand add `meet.google.com`, open a Meet
  link, see it open in Chrome.
- [ ] **Step 5: Commit** `feat: open in chrome with per-site rules`

### Task 7: DevTools through Servo's remote debugging

**Files:**
- Modify: `crates/engine/src/servo_engine.rs`, `crates/wren/src/{app.rs,window.rs}`
- Create: `docs/devtools.md` (en) and `site/content/docs/devtools.mdx` + `.es.mdx`

**Interfaces:**
- Engine: `Engine::set_devtools(&mut self, port: Option<u16>) -> anyhow::Result<()>`
  (Servo preference `devtools_server_enabled` and
  `devtools_server_listen_address = 127.0.0.1:<port>`; only loopback).
- GPUI action `devtools::Toggle` (`cmd-alt-i`); a visible "DevTools
  on :7000" indicator in the toolbar while enabled.

- [ ] **Step 1: Write failing test** `devtools_toggle_shows_indicator`
  (FakeEngine records `set_devtools(Some(7000))`, the toolbar shows the
  indicator; toggling again records `None`).
- [ ] **Step 2: Run** `cargo nextest run -p wren`. Expected: FAIL.
- [ ] **Step 3: Implement.** If Servo only reads the preference at
  start, restart-free toggling is not possible: then the setting applies
  on next launch and the indicator says so. Document connecting from
  Firefox (`about:debugging`, Network location `localhost:7000`).
- [ ] **Step 4: Run** test; by hand inspect a page from Firefox DevTools.
- [ ] **Step 5: Commit** `feat: devtools via servo remote debugging`

### Task 8: Top-sites harness and the must-work smoke tests

**Files:**
- Modify: `crates/parity/src/{lib.rs,bin/parity.rs}`; create `crates/parity/src/topsites.rs`,
  `crates/parity/data/topsites.txt` (200 hosts), `crates/parity/data/must_work.toml`
- Modify: `crates/wren/src/cli.rs` (`--harness <out.json>`)
- Test: `crates/parity/tests/topsites.rs`

**Interfaces:**
- `wren --harness <out.json> --profile-dir <dir> URL...`: loads each URL
  in its own tab one at a time, waits for `Load{Complete}` or 20 s,
  writes per URL `{ url, final_url, title, load_ms, console_errors, screenshot_png, crashed }`
  and exits.
- `parity topsites --out <dir>`: runs the harness for Wren and headless
  Chrome (`--headless=new --screenshot`), diffs screenshots
  (`image-compare` SSIM), writes `docs/research/perf/<date>-topsites.md`
  with usable / broken per site (usable = loaded, not crashed, SSIM
  >= 0.6 or the must-work probe passes).
- `must_work.toml`: Google Calendar, Google Meet, Slack (web), each with
  a URL and a JS probe (an element selector that exists when logged in);
  run with `parity topsites --must-work --profile-dir <owner profile>`.

- [ ] **Step 1: Write failing tests**: `harness_json_schema` (parse a
  sample report), `usable_rule` (table of inputs), `must_work_toml_parses`.
- [ ] **Step 2: Run** `cargo nextest run -p parity`. Expected: FAIL.
- [ ] **Step 3: Implement**; CI runs the public top-sites list weekly
  (`parity.yml` schedule), never the must-work list (needs logins).
- [ ] **Step 4: Run** the harness on the first 50 sites locally and commit
  the report; run the must-work list with the owner's profile.
- [ ] **Step 5: Commit** `feat(parity): top-sites harness and must-work smoke tests`

### Task 9: App bundle and milestone exit

**Files:**
- Create: `scripts/bundle.sh`, `assets/macos/Info.plist`, mise task `bundle`
- Modify: `docs/research/perf/`, roadmap status

- [ ] **Step 1:** `mise run bundle` builds release, assembles
  `target/Wren.app` (Info.plist with bundle id `io.github.oscarbc96.wren`,
  icon from `assets/`), `codesign -s - --deep`.
- [ ] **Step 2:** RAM benchmark: Wren release vs Chrome, tabs10 and
  tabs20, results under `docs/research/perf/`. No regression above 10%
  against M0.
- [ ] **Step 3:** Must-work exit: Calendar, Slack and DevTools usable in
  Wren; Meet usable in Wren or routed to Chrome by rule.
- [ ] **Step 4: Commit** `chore: m1 exit, bundle and measurements`
