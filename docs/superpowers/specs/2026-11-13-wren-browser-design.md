# Wren: a low-RAM, AI-native desktop browser in Rust

Status: draft for review, 2026-11-13. "Wren" is a working name (placeholder).

## 1. Intent and why

Chrome is the default browser for most people and it is heavy: hundreds of
MB idle, well over 1 GB with a normal tab load. Every serious alternative is
either Chromium underneath (Brave, Edge, Arc, Dia) or not yet usable day to
day (Servo's demo shell, Ladybird alpha). Nobody ships a maintained,
full-featured browser on the Rust Servo engine; the last attempt (Verso)
was archived in Oct 2025 because it could not keep pace with Servo.

Wren is that browser: a product shell around the Servo engine, with the
features people need to leave Chrome (extensions, password managers,
DevTools) and an AI layer that is designed in, not bolted on.

Who it is for, in order:
1. The owner, as an everyday browser (dogfooding drives priorities).
2. An open-source product others can use.
3. Later, a commercial layer on top (hosted models, sync, managed agents)
   that plugs into the open core and is never required by it.

## 2. Goals

- **Daily-driver browser on Servo**, macOS first.
- **Low RAM as a measured, public claim**, benchmarked against Chrome on the
  same workloads every release.
- **Chrome parity tracked, not guessed**: web-platform-tests, top sites,
  top extensions and RAM, all versus Chrome, in CI with a published
  dashboard.
- **Chrome extensions** (Manifest V3) installed from the Chrome Web Store,
  growing API coverage by demand.
- **Password managers**: system passkeys and passwords (iCloud Keychain and
  third-party credential providers) plus 1Password/Bitwarden.
- **Chrome DevTools** usable against Wren tabs; Playwright and Puppeteer
  can drive Wren over the Chrome DevTools Protocol (CDP).
- **WebAssembly** for web content (free from Servo) and a **Wasm plugin
  system** for native Wren add-ons.
- **AI-native**:
  - bring-your-own LLM providers (OpenAI-compatible, Anthropic,
    OpenRouter, local Ollama);
  - a built-in agent that reads and acts on pages and across tabs;
  - **WebMCP** support so sites can expose tools to agents;
  - an MCP server and CDP so outside agents (Claude Code etc.) can drive
    Wren;
  - **generated pages**: address-bar questions answered by a page the AI
    builds with OpenUI components from live search;
  - **pinned generated pages** that refresh their data without calling the
    model again;
  - **routines**: detected or user-defined automatic tasks (e.g. "my usual
    Friday pizza order"), with confirmation before anything consequential.

## 3. Non-goals

- Writing a new web engine. Servo is the engine; fixes go upstream.
- 100% Chrome extension compatibility. Extensions that rely on Chromium
  internals (e.g. chrome.debugger internals, NaCl, enterprise policies)
  are out.
- Embedding Chromium (CEF) as a second engine.
- NPAPI or other legacy plugins.
- Mobile in v1. Windows and Linux come after macOS, not in v1.
- Our own sync service and hosted models in the open-source core.

## 4. Anti-goals (failure even if it "works")

- RAM ends up close to Chrome's. The reason to exist is gone.
- A fork of servoshell that has to be hand-merged every Servo release
  (the Verso failure mode).
- The AI acts with money, messages or accounts without explicit user
  confirmation, or page content can steer the agent (prompt injection).
- Browsing history or routine detection leaves the machine without consent.
- The open core depends on a paid service to be useful.
- Generated pages that state facts without sources.

## 5. Constraints

- Language: Rust. License: **MPL-2.0** (owner decision 2026-11-13), docs CC-BY-4.0, with a
  CLA. Reason: same licence as Servo, Firefox and Zen; file-level copyleft
  keeps changes to Wren's files open while letting companies embed and
  contribute. Template code reused here is relicensed by its author. UI is
  GPUI-CE (Apache-2.0) with Wren's own components. The CLA keeps dual licensing open for the commercial
  layer. Full review in section 5b.
- Repo, tooling and process copied from the template (section 5c): mise, hk, GPUI,
  `.agents/`, release-please, Astro site.
- Engine: Servo embedded as a library through its public embedding
  (WebView) API, pinned to a released crates.io version (`servo =0.8.0` as of 2026-11-13) and
  bumped deliberately.
- **Servo patch queue (owner decision)**: when Wren needs a hook Servo
  does not expose (script injection and isolated worlds, network
  interception, accessibility tree, CDP data, form state), Wren carries a
  small patch series on its pinned Servo instead of waiting. Rules: each
  patch is minimal, lives in a `wren-servo` branch as a separate commit,
  has an upstream PR opened at the same time, and is dropped when upstream
  lands it. The series is rebased on every Servo bump. Features do not
  wait for upstream.
- Platform v1: macOS 14+ on Apple Silicon.
- Keys and secrets live in the macOS Keychain, never in plain files.
- Public-repo hygiene: commits and releases under a project/bot identity.

## 5a. Prior art (checked 2026-11-13)

| | Engine | Open source | AI | Extensions | RAM angle |
|---|---|---|---|---|---|
| Perplexity Comet | Chromium | No | Agent that acts (shopping, email, groceries), research | Chrome | None |
| Zen | Firefox fork | Yes, MPL-2.0 (~46k stars) | None notable | Firefox add-ons | Firefox's |
| SigmaOS | WebKit, SwiftUI, macOS only | No | Airis chat, multi-page "Look it up", Simplify; paid tiers $20-30/mo | Chrome Web Store on WebKit | Page suspension |
| Arc / Dia | Chromium | No | Dia: page and tab chat | Chrome | None |

What Wren takes: Zen's open, community model and calm tab UX; SigmaOS's
proof that Chrome extensions on a non-Chromium engine plus aggressive
page suspension is viable; Comet's "assistant that acts" bar for the agent.

Where Wren is different: only open-source, non-Chromium, AI-native
option; Rust/Servo with RAM as a published benchmark; WebMCP, MCP and CDP
for external agents; generated pages that stay live without the model;
local-only routine detection; bring-your-own models with no paywall in
the core.

## 5b. License review (2026-11-13)

Wren is MPL-2.0. Every component below must be compatible with
that and ship its notices in `NOTICE` and `THIRD_PARTY_LICENSES.html`
(`cargo about`). `cargo deny` uses the template's allow list.

| Component | License | Status | Notes |
|---|---|---|---|
| Servo, Stylo, WebRender, SpiderMonkey (mozjs) | MPL-2.0 | OK | Same licence as Wren. Our Servo patch queue is MPL and public |
| servo-media | MPL-2.0 | OK | |
| GStreamer core and plugins | LGPL-2.1-or-later | OK with care | Link dynamically, ship as dylibs in the bundle. Ship only `base`, `good` and the needed `bad` plugins; exclude GPL plugins (`ugly`, x264) and patent-encumbered codecs. Meet uses VP8/VP9/AV1 and Opus (royalty-free); H.264/AAC through macOS VideoToolbox/AudioToolbox, whose patents Apple covers |
| GPUI-CE (`gpui-ce`, `gpui_ce_platform`, `gpui_ce_elements`, crates.io 0.3.0) | Apache-2.0 | OK | Community fork of Zed's GPUI; Apache-2.0 is MPL-compatible. Zed's GPL `ui`/`theme` crates are not used |
| Template code (settings, mcp, plugin host, scripts, site) | relicensed to MPL-2.0 by its author | OK | Same owner |
| Obscura (crates, reader, `obscura-cdp`) | Apache-2.0 | OK | Vendored `cosmic-text`, `taffy` are MIT/Apache; `wreq-util` to verify. V8 is BSD-3-Clause, OK |
| Obscura stealth build (wreq + BoringSSL) | BoringSSL: ISC plus legacy OpenSSL license | **Avoid** | The legacy OpenSSL license is GPL-incompatible. Use Obscura's rustls build; Wren needs no fingerprint spoofing |
| Brave `adblock` | MPL-2.0 | OK | |
| wasmtime, wit-bindgen | Apache-2.0 WITH LLVM-exception | OK | |
| rmcp | Apache-2.0 | OK | |
| OpenUI (renderer, lang-core) and React | MIT | OK | |
| Chrome DevTools frontend | BSD-3-Clause | OK | Bundled; keep its notices |
| Chromium extension API schemas (IDL/JSON), if copied for types | BSD-3-Clause | OK | |
| Fonts and icons | OFL / open licenses only | OK | As in the template |

Not code licenses, still to respect:
- **Trademarks**: never use "Chrome", "Google", "Servo", "Zed" or "GPUI" marks in
  the product name or logo. "Open in Chrome" and "works with Chrome
  extensions" are descriptive use only.
- **Name "Wren"**: a programming language (MIT) shares it; run a
  trademark search before public alpha.
- **Chrome Web Store**: Wren downloads extensions on the user's request
  and does not redistribute them; each extension keeps its own license.
  Re-read Google's terms before public alpha (as in section 12).
- **Search APIs**: several (e.g. Brave Search API) restrict storing or
  caching results. Pinned generated pages re-query; they never keep raw
  results beyond what the provider's terms allow.
- **MPL scope**: others may build closed products that include Wren, but
  changes to Wren's own files must be published under MPL-2.0. Commercial
  layers (console, hosted agents) can be closed either way.

## 5c. Repo and tooling (copied from the template)

Copy everything that is not product-specific from the owner's template
repo (path given in task briefs, never written here), renaming its binary,
env prefix and URI scheme to `wren`, `WREN_*`, `wren://`:
- `mise.toml` tools and tasks (`build`, `run`, `test`, `test:perf`, `lint`,
  `fmt`, `bundle`, `dev:cert`, `site:*`, `lint:pr-body`), `hk.pkl` (same
  builtins, gitleaks, commit-msg, agents-symlink check),
  `rust-toolchain.toml`, `typos.toml`, `.editorconfig`, `deny.toml`,
  `.github/` (CI, dependabot, PR template, pages, release-please),
  `scripts/dev-cert.sh`, `scripts/dev-sign.sh`, `scripts/pr-body.ts`.
- Docs and policy: `AGENTS.md` (CLAUDE.md symlink), `.agents/`,
  `CLA.md`, `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`, `SECURITY.md`,
  `PRIVACY.md`, `TERMS.md`, `TRADEMARKS.md`, `NOTICE`.
- Crates worth lifting: `settings`, `themes`, `components` (generic
  parts), `mcp`, `plugin_host`, plugin API crate (as `wren_plugin_api`),
  the template's wry child-view code as the model for hosting Servo's view.
- Site: Astro + Fumadocs on Bun with the same i18n (English + Spanish).
- Conventions: unprefixed snake_case crates, binary crate `wren`,
  Conventional Commits, self-merged squash PRs, local green is the gate,
  signed commits.
- UI framework: **GPUI-CE** from crates.io (`gpui-ce`, `gpui_ce_platform`,
  `gpui_ce_elements`, pinned exactly), not Zed's GPUI. Template code that
  wraps Zed's GPL `ui`/`theme` crates is not copied; Wren writes its own
  `themes` (reading Zed-style theme JSON, implemented from scratch) and
  `components`.

## 6. Approaches considered

| | Approach | Trade-off | Verdict |
|---|---|---|---|
| A | Own Rust shell embedding Servo as a library | Lowest RAM, clean upgrade path; Servo compatibility gaps stay until fixed upstream | **Chosen** |
| B | Fork servoshell and grow it | Fast first demo; fork churn every Servo release killed Verso | Rejected |
| C | Servo default plus Chromium (CEF) fallback per site | Every site works; Chromium tabs bring Chrome RAM back, two engines to maintain | Rejected |

Obscura (Rust headless browser, V8 + own DOM, CDP) was evaluated as the
engine and rejected for that role: no GPU compositing, no media, no
process isolation, CPU raster only. It is used where it is strong: a
headless background reader for the AI, and its CDP code as a starting point
for Wren's CDP server.

Compatibility gap mitigation for A: an "Open in Chrome" action per site,
plus a per-site compat list that remembers it.

## 7. Architecture

``
+--------------------------------------------------------------+
| wren (binary): GPUI app, windows, menus, lifecycle, updater   |
|  +------------------+   +----------------------------------+ |
|  | GPUI chrome      |   | internal pages (wren://)         | |
|  | tabs, omnibox,   |   | settings, history, extensions,   | |
|  | sidebar (GPUI-CE) |   | AI sidebar, generated pages      | |
|  +------------------+   +----------------------------------+ |
|  tabs: tab lifecycle, sleep/freeze, memory budget             |
|  engine: Servo embedding (WebView per tab, child NSView)      |
+------------------+-------------------+-----------------------+
| extensions       | cdp               | creds                 |
| chrome.* runtime | CDP server        | passkeys, Keychain,   |
| CWS installer    | (DevTools, PW)    | 1Password/Bitwarden   |
+------------------+-------------------+-----------------------+
| ai: providers | agent | webmcp | mcp                          |
| genui (OpenUI) | reader (Obscura) | routines                  |
| plugin_host (wasmtime), wren_plugin_api                       |
+--------------------------------------------------------------+
| store: profile DB (SQLite), history, pins, recipes            |
| settings, themes, components (from the template)                      |
| parity: WPT/site/extension/RAM harness (CI only)              |
+--------------------------------------------------------------+
``

Crate names follow the template's convention (unprefixed snake_case; `wren_`
only for globally named things). Crate split is a proposal; the builder
may merge small crates.

### 7.1 Shell and UI

- Browser UI (toolbar, tab strip, omnibox, sidebar, menus, command
  palette) in **GPUI-CE** with Wren's own `components`. Settings and
  keymap loading reuse the template's crates; `themes` is Wren's own.
- Each tab's Servo WebView renders into a **child NSView** inside the GPUI
  window, the same pattern the template uses for its `wry` child webview. Servo
  gets the view through `raw-window-handle`; GPUI owns the event loop and
  forwards input and wake-ups to Servo. This is the first spike of M0
  (risk: GL surface from Servo next to GPUI's Metal layer; fallback is
  Servo rendering offscreen into a shared IOSurface that GPUI draws).
- Everything panel-like (settings, history, downloads, extension
  management, AI sidebar, generated pages) is an internal `wren://` page
  rendered by Servo. One rendering stack, low RAM, and these pages double as
  dogfood for Servo compatibility.
- `wren://` pages get a privileged bridge (`window.wren.*`) to call Rust;
  web pages never see it.

### 7.2 Tabs and memory

The low-RAM claim is mostly won here:
- Memory budget per profile, default target below in 8.1.
- Background tabs move through states: active, backgrounded (timers
  throttled), slept (WebView torn down; URL, scroll and form state kept),
  frozen to disk.
- Tabs playing media, with unsaved form input, or pinned by the user
  are never slept.
- Ad and tracker blocking built in with Brave's Rust `adblock` crate
  (fewer bytes and scripts loaded is the cheapest RAM saving).
- Process model: **one content process per tab** (owner decision,
  2026-11-13), so a tab that hangs or crashes can be killed and reloaded
  without touching the others. Wren runs Servo in multiprocess mode: Servo
  keys event loops by browsing context group and site, and every tab Wren
  opens is its own group, so each tab gets its own process (cross-site
  iframes may add more; popups opened with an opener share the opener's
  tab process, as in Chrome). The `wren` binary doubles as the content
  process (`servo::run_content_process` on a hidden flag).
- Process control: a task manager (`cmd-alt-esc`, also `wren://processes`)
  lists every process with its tab, RSS and CPU, and has End process.
  Killing a tab process shows that tab's crash notice with Reload; other
  tabs are untouched. A hang watchdog flags a tab whose script thread has
  not answered for 5 s (Servo's background hang monitor) and offers Wait /
  End process in the tab.
- RAM cost: each content process costs a fixed overhead. Sleeping tabs
  (above) exit their process, so idle tabs cost no process. Measure the
  per-process overhead in M0 and keep the 20-tab target; if it is missed,
  fall back to one process per site, not to a single process.

### 7.3 Parity harness (`parity`)

Runs in CI on a schedule and per release, results published to a static
dashboard in the repo's GitHub Pages:
1. **Web-platform-tests**: run WPT against Wren's Servo build and against
   Chrome stable; report pass rate per area and the delta. Upstream wpt.fyi
   data is a cross-check.
2. **Top sites**: a list of ~200 popular sites; load in Wren and headless
   Chrome; compare screenshots (perceptual diff), console errors and key
   interactions (login page renders, search box works). Failures are triaged
   into "Servo bug, filed upstream", "Wren bug", or "needs site quirk".
3. **Top extensions**: the top 100 Chrome Web Store extensions; install,
   run a smoke script each; pass/fail per extension and the missing
   `chrome.*` APIs that blocked it. This list sets the order of API work.
4. **RAM and speed**: fixed workloads (1 tab idle; 10 and 20 typical tabs;
   a 1-hour soak) measuring RSS of all processes, versus Chrome. A
   regression past threshold fails the release.

### 7.4 Chrome extensions (`extensions`)

Wren implements the WebExtensions/`chrome.*` API itself, as Firefox and
Orion do.
- **Install**: user clicks install from a Wren page or pastes a Web Store
  URL; Wren downloads the CRX from the Web Store update endpoint, verifies
  the signature, unpacks, shows permissions, installs. Updates use the same
  endpoint. Unpacked "developer mode" install is supported.
- **Pieces**: content scripts and CSS injection; isolated worlds; background
  service worker (runs in a hidden WebView; depends on Servo service worker
  support, which needs upstream work, with an event-page fallback);
  action popup and options pages as small WebViews; messaging between all
  of them.
- **API order** (driven by the extension harness): `runtime`, `storage`,
  `tabs`, `scripting`, `i18n`, `action`, `contextMenus`, `alarms`,
  `notifications`, `declarativeNetRequest` (backed by the adblock engine),
  `webNavigation`, `cookies`, `identity`, `sidePanel`.
- **Target for 1.0**: password-manager extensions (1Password, Bitwarden),
  uBlock Origin Lite, Dark Reader, and at least 60 of the top 100
  passing the smoke harness.

### 7.5 Password managers and passkeys (`creds`)

- **Passkeys (WebAuthn)**: route `navigator.credentials` to macOS
  AuthenticationServices so the system sheet offers iCloud Keychain and
  any third-party passkey provider the user has enabled (1Password,
  Bitwarden). Needs Apple's browser passkey entitlement, which needs an
  Apple Developer account. **There is no organization account today**, so
  passkeys are not on M1's critical path: M1 uses the built-in Keychain
  password store and Chrome import; 1Password/Bitwarden arrive with
  extensions in M2; passkeys land once an account and entitlement exist.
- **Passwords**: a built-in password store kept in the macOS Keychain, with
  import from Chrome and CSV; system AutoFill from enabled credential
  providers where macOS allows it for browsers (to verify on macOS 15+).
- **1Password/Bitwarden extensions** through `extensions` as the main path for
  their full feature set.

### 7.6 DevTools and automation (`cdp`)

- Servo natively speaks Firefox's remote debugging protocol. Wren adds a
  CDP server, starting from Obscura's `obscura-cdp` crate (Apache-2.0,
  attributed), re-targeted from Obscura's DOM to Servo's embedding API.
- Chrome's open-source DevTools frontend is bundled and opened as a
  `wren://devtools` page against that server.
- Domains in order: Target, Page, Runtime, DOM, Network, Input, Console,
  then CSS, Debugger, Performance as Servo exposes the data.
- Same server lets Playwright and Puppeteer `connectOverCDP` to Wren. Off
  by default; enabled per session with a visible indicator.

### 7.7 WebAssembly and plugins

- Web content Wasm: provided by Servo's SpiderMonkey. Tracked in WPT.
- **Wren plugins**: native add-ons as WebAssembly components run in
  wasmtime with explicit capabilities (read page, add sidebar panel,
  register agent tool, register search provider, register generated-page
  components). For things the `chrome.*` API cannot express, and as the
  extension point for AI tools.

## 8. AI layer

### 8.1 Providers (`ai`)

- Provider interface with: chat streaming, tool calling, structured output.
- Built in: OpenAI-compatible (covers many), Anthropic, OpenRouter, Ollama.
  Keys in Keychain. Per-feature model choice (fast model for page Q&A,
  stronger one for agent tasks).
- The commercial layer later is "just another provider" plus optional
  services; no code path requires it.

### 8.2 Agent (`agent`)

- Tools: read page (accessibility tree + main-text extraction), find,
  click, type, scroll, navigate, open/close/switch tabs, screenshot,
  call WebMCP tools, call Wren plugin tools, search, read URL in background.
- Prefers structured paths: WebMCP tools first, then accessibility tree
  actions, screenshots last.
- **Safety rules** (non-negotiable defaults):
  - page and tool output is data, never instructions; the agent's system
    prompt and a separate check flag instruction-like content from pages;
  - consequential actions need user confirmation: payments, purchases,
    sending messages or email, submitting forms on new sites, account or
    security settings, downloads of executables;
  - per-task site scope (allowed origins) and mode (read-only or act);
  - every agent action logged in a per-task timeline the user can review.
- **Surfaces**: AI sidebar (ask about this page, these tabs), omnibox,
  routines (8.5), external control via `mcp` and `cdp`.

### 8.3 WebMCP (`webmcp`)

- Implement `document.modelContext.registerTool()` / `unregisterTool()`
  per the W3C Community Group draft (the `navigator.modelContext` alias is
  deprecated in Chrome 150; support it only as a shim if cheap).
- HTTPS only, per document, cleared on navigation.
- Registered tools appear to the Wren agent and to external agents via
  `mcp`, labelled with their origin; calling a tool that changes state
  follows the confirmation rules in 8.2.
- Track spec changes; it is a draft and has already changed once in 2026.

### 8.4 Generated pages (`genui`)

Address bar routing: something that parses as a URL navigates; a question or
task produces a generated page (default, can be switched off for "plain
search"); a modifier key forces either.

Pipeline:
1. Agent plans queries and calls the configured **search provider**
   (Brave Search API, Kagi, Exa, SearXNG self-hosted; pluggable).
2. Top results are read by **`reader`**: Obscura embedded as a headless
   reader (no tabs, no rendering, about 30 MB), returning clean text and
   metadata. Pages that need full rendering fall back to a hidden Servo
   WebView.
3. The model writes **OpenUI Lang** against Wren's component library,
   streaming.
4. A `wren://gen/<id>` page renders it progressively with OpenUI's
   renderer and Wren's components (cards, tables, charts, comparisons,
   maps, forms, source lists, image galleries).

Rules:
- Every factual block cites sources; clicking one opens the real page.
- Generated pages run sandboxed: no direct network, only registered
  components, no arbitrary script. Data comes only through the tool
  provider bridge.
- Follow-up questions edit the page incrementally (OpenUI returns only
  changed statements).
- OpenUI is single-vendor (Thesys, MIT). `genui` keeps a format
  interface so the generator format can change (e.g. A2UI) without touching
  the rest.

### 8.5 Pinned and live generated pages

- OpenUI Lang supports `Query("tool", args)`, `Mutation(...)`, `$state`
  and `Action([@Run(...)])`. After generation the runtime calls tools via a
  `toolProvider` with **no model call**.
- Wren's toolProvider maps tool names to Wren tools: search, reader fetch,
  site WebMCP tools, plugin tools, user-connected accounts.
- **Pin** saves the OpenUI program, not a snapshot. Refresh happens on
  open and on a schedule set by Wren (OpenUI has no polling syntax). The
  model is only called when the user asks to change the page.
- Pinned pages live in the sidebar and can be the output of routines
  ("morning briefing", "flight prices BCN-LON").

### 8.6 Routines (`routines`)

- **Detection (local only)**: a local job looks for repeats in history:
  same origin, similar weekday/time, similar cart contents, checkout
  reached. When confident, it offers: "You order from X most Fridays around
  20:00. Make this a routine?" No history leaves the machine for detection;
  any model call for it uses a local model or explicit consent.
- **Recipe**: a routine stores intent (items, options, address), the site,
  the recorded steps, and preferred WebMCP tools if the site offers them.
- **Run**: agent opens the site in a hidden or visible tab, rebuilds the
  cart (WebMCP first, recorded steps second, agent reasoning third).
  On any drift (item missing, price change above a threshold, layout
  changed) it stops and asks.
- **Confirmation**: payment always needs approval by default ("Cart ready:
  2 pizzas, EUR 27.40. Confirm?") via a macOS notification or in-app. The
  user may set a spending cap for auto-confirm per routine.
- **Triggers**: schedule, suggestion only, page change, or "do my usual"
  in the omnibox. Also user-written routines in natural language ("every
  morning at 8 summarize these 5 sites").
- Routines need the app running; a lightweight menu-bar agent keeps the
  scheduler alive with the window closed.

## 9. Data and privacy

- One profile DB (SQLite) for history, bookmarks, pins, recipes, agent
  logs. Secrets in Keychain only.
- Telemetry off by default; crash reports opt-in.
- AI calls show which provider and which data (page, tabs) is sent.

## 10. Milestones

Each ends with something usable and measured.

1. **M0 Shell**: window, tabs, omnibox, history, bookmarks, embedded Servo,
   one content process per tab (Servo multiprocess) with crash recovery.
   RAM benchmark and WPT harness running in CI.
2. **M1 Daily driver**: task manager with End process and the hang
   watchdog, tab sleep/freeze, adblock, built-in Keychain
   passwords + Chrome import, downloads, "Open in Chrome", top-sites
   harness, DevTools via Servo's existing Firefox remote-debugging support.
   **Owner's must-work list (exit gate)**: Google Calendar, Google Meet,
   Slack (web), DevTools. Each gets a scripted smoke test in the top-sites
   harness. Gaps are fixed through the Servo patch queue. Meet is the
   biggest risk (WebRTC, camera and microphone, screen share in Servo); if
   Meet is not usable, it opens in Chrome via a per-site rule and M1 still
   exits. Builds are ad-hoc signed for local use.
3. **M2 Chrome DevTools**: CDP server, bundled Chrome DevTools frontend,
   Playwright/Puppeteer support (moved up because the owner needs
   DevTools daily).
4. **M3 Extensions core**: CWS install, content scripts, storage, runtime,
   tabs, action, popup; 1Password/Bitwarden + uBO Lite + Dark Reader.
   Extension harness.
5. **M4 AI core**: providers, AI sidebar, agent with safety rules, `mcp`,
   WebMCP.
6. **M5 Generated pages**: search providers, `reader`, OpenUI pipeline,
   pin and refresh.
7. **M6 Routines**: detection, recipes, scheduler, confirmations.
8. **M7 Plugins**: wasmtime plugins.
9. **Public alpha**: signed, notarized macOS build, auto-update, docs,
   parity dashboard public, passkeys. Needs an Apple Developer account
   (none exists yet; personal or organization is an open decision before
   this milestone).

First run without keys: the omnibox does plain search with a default
engine until the user adds a model and a search provider; local Ollama
plus self-hosted SearXNG is the documented free path.

## 11. Targets (initial, revisit after M0 measurements)

- RSS counts every Wren process (content, internal pages, extension
  WebViews, the Obscura reader, the menu-bar agent). The reader runs as a
  separate on-demand process that exits when idle. Chrome is measured
  clean, and again with the same extensions where Wren supports them.
- 1 tab idle: at most 50% of Chrome's RSS on the same machine.
- 20 typical tabs: at most 50% of Chrome's RSS.
- WPT: within the Servo release's own pass rate (Wren adds no regressions).
- Top 200 sites: 90% "usable" by M1 exit, tracked weekly.
- Top 100 extensions: 60% pass by 1.0.

## 12. Risks

- **Servo compatibility** is the product's ceiling. Mitigation: harness
  tells us what breaks, upstream fixes, Open in Chrome.
- **Servo service workers and multiprocess** maturity gate MV3 background
  scripts and isolation. Mitigation: event-page fallback; contribute.
- **Servo embedding API churn**. Mitigation: one adapter crate
  (`engine`) owns all Servo calls.
- **Apple account and entitlement**: none yet; passkeys and notarized
  releases wait for it. Decide account type before public alpha.
- **Google Meet in Servo** (WebRTC, media capture) may need large patches.
  Mitigation: per-site Open in Chrome.
- **Servo patch queue growth**: if the series grows past a handful of
  patches it becomes the fork the anti-goals warn about. Review its size
  at every Servo bump.
- **Web Store terms** for third-party browsers installing CRX files.
  Others (Brave, Edge, Orion) do it; check terms before public release.
- **WebMCP and OpenUI are young** and may change. Both behind interfaces.
- **Agent safety**: prompt injection and unintended purchases. Defaults in
  8.2 and 8.6 are non-negotiable; red-team before public alpha.
- **Scope**: this is a multi-year product. Milestones are ordered so the
  owner can switch at M1 even if later ones slip.

## 13. Decided vs left to the builder

Decided by the owner after review: Servo patch queue allowed, features do
not wait for upstream; one content process per tab from M0 (2026-11-13,
replacing the earlier single-process decision); M1 must-work list is Google Calendar, Meet, Slack and
DevTools; no Apple organization account exists.

Build setup (Claude's call): planned in detail for M0 and M1, later
milestones as outlines; built by the owner with AI coding agents.

Decided in conversation: approach A (Servo embedded, own shell); everyday
browser first, then open-source product, commercial layer later; Chrome
extension support; native password managers; Chrome DevTools; Wasm and
plugins; BYO LLM providers; WebMCP; agent and external control; generated
pages with OpenUI by default with live search; pinned pages that refresh
without the model; routines including detected ones with payment
confirmation.

Approved by the owner 2026-11-13, with: copy everything possible from
the template (mise, hk, process); review all licenses (section 5b). UI on
GPUI-CE (owner, 2026-11-13). MPL-2.0 is the owner's licence choice (2026-11-13); reused template code
is relicensed by its author.

Proposed by Claude (the owner may override): name "Wren";
macOS first; GPUI chrome plus `wren://` internal pages; Obscura as
background reader and CDP starting point; Brave `adblock` crate; wasmtime
plugins; search providers list; RAM targets in section 11; milestone order.

Left to the builder: crate boundaries, SQLite schema, Servo version pin,
WPT subset selection and CI runner sizing, perceptual-diff thresholds,
component library design for generated pages, detection heuristics for
routines.
