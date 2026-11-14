# M5 Generated Pages Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** typing a question in the address bar produces a cited,
interactive page at `wren://gen/<id>`, built from search results read by
a light headless reader, which the user can refine, pin, and have
refreshed without new model calls.

**Architecture:** omnibox routing decides URL vs search vs generate.
`search` holds pluggable search providers. `reader` is a separate
on-demand helper process (`wren-reader`) built on Obscura's crates with
the rustls build: it fetches and extracts pages and exits when idle;
pages that need full rendering fall back to a hidden Servo webview
(M3 `create_hidden`). `genui` runs the pipeline (plan queries, search,
read, generate OpenUI Lang with the M4 `ai` provider, stream), keeps a
`Format` trait so OpenUI can be swapped, and stores programs. The
`wren://gen/<id>` page is a static bundle (built with Bun from OpenUI's
MIT `@openuidev/react-lang` renderer, React and Wren's component
library) served from the `wren` binary; it talks to Rust only through a
`toolProvider` bridge over the M3 scheme RPC (host `wren-gen`), so the
page has no network access of its own.

**Tech Stack:** as M4, plus Obscura crates (`obscura-browser`,
`obscura-net`, Apache-2.0, rustls build, no `stealth` feature),
`@openuidev/react-lang` + `@openuidev/lang-core` (MIT), React (MIT),
Bun bundler.

**Spec:** `docs/superpowers/specs/2026-11-13-wren-browser-design.md`
(sections 1, 5b, 8.4, 8.4a, 8.5, 9, 10 M5, 11 reader RSS rule).
Roadmap: `docs/superpowers/plans/2026-11-13-roadmap.md`.

## Global Constraints

- All M0 to M4 constraints hold.
- Routing: input that parses as a URL (scheme, or host with a dot and
  no spaces, or `localhost[:port]`) navigates; anything else generates
  when `omnibox.generate = true` (default `true` only once a model and a
  search provider are set; otherwise plain search with
  `search.default_engine`). `alt-enter` forces plain search, `cmd-enter`
  forces generate.
- Search providers: Brave Search API, Kagi, Exa, SearXNG (base URL).
  Keys in the Keychain (`io.github.oscarbc96.wren.search`). None
  configured by default. Documented free path: local Ollama plus
  self-hosted SearXNG.
- Reader: separate process, started on demand, exits after
  `reader.idle_seconds` (default 60), never keeps cookies, rustls only
  (the BoringSSL stealth build is forbidden by the licence table), same
  SSRF guard as Obscura (`obscura-ssrf`: no private, loopback or
  link-local targets). Its RSS counts in the RAM benchmark.
- Generated pages: no network, no `eval`, only registered components,
  CSP `default-src 'self' wren:; script-src 'self'; connect-src
  chrome-extension://wren-gen` (the RPC host). Every factual block
  carries sources; clicking a source opens the real page in a new tab.
- Pinned pages store the OpenUI program and its tool bindings, never a
  rendered snapshot. Refresh runs tools only (no model call): on open,
  and on the pin's schedule (`refresh_minutes`, default off, minimum
  15). The model runs only when the user asks to change the page.
- Before any model call the omnibox dropdown shows the provider and
  what is sent (the query only).

## Review Focus

1. **`localhost:3000`, `192.168.1.1`, `foo.local`, and `what is 2+2`**
   must route as URL, URL, URL and generate. Pinned by
   `routing_table` (Task 1).
2. **A search result pointing at `http://127.0.0.1/admin` or
   `http://169.254.169.254/`** must never be fetched by the reader.
   Pinned by `reader_refuses_private_targets` (Task 2).
3. **Generated OpenUI that names an unknown component or tries
   `<script>`** must render an inline error block, not execute anything
   and not blank the page. Pinned by `unknown_component_renders_error`
   (Task 4).
4. **A pinned page refresh whose tool fails** (search quota, site down)
   must keep the last good data with a "refresh failed" note, not empty
   the page. Pinned by `failed_refresh_keeps_last_data` (Task 5).
5. **A follow-up edit** must change only the statements OpenUI returns
   and keep the rest (including the user's `$state`). Pinned by
   `follow_up_patches_only_changed_statements` (Task 4).

---

### Task 1: Omnibox routing and `search` providers

**Files:**
- Create: `crates/search/{Cargo.toml,src/lib.rs,src/brave.rs,src/kagi.rs,src/exa.rs,src/searxng.rs}`
- Modify: `crates/omnibox/src/*` (routing), settings schema + snapshot, `crates/wren/src/window.rs` (dropdown line)
- Test: `crates/omnibox/tests/routing.rs`, `crates/search/tests/providers.rs` (local mock server)

**Interfaces:**
- `pub enum Route { Navigate(Url), Search(String), Generate(String) }`, `omnibox::route(input: &str, generate_enabled: bool, modifier: Modifier) -> Route`.
- `pub struct SearchResult { title, url: Url, snippet, published: Option<String> }`; `#[async_trait] pub trait SearchProvider { fn id(&self) -> &str; async fn search(&self, q: &str, n: usize) -> Result<Vec<SearchResult>, SearchError>; }`.
- Settings: `search.provider` (`""` | `brave` | `kagi` | `exa` | `searxng`), `search.searxng_url`, `search.default_engine` (plain search URL template, default DuckDuckGo `https://duckduckgo.com/?q=%s`), `omnibox.generate`.

- [ ] **Step 1: Write failing tests**: `routing_table` (the Review Focus cases plus `https://x`, `x.com`, `x com`, `?x` forcing search), `modifiers_force_route`, per provider `parses_results` and `auth_error_is_clear`.
- [ ] **Step 2: Run** `cargo nextest run -p omnibox -p search`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** tests; screenshot of the omnibox dropdown showing "Generate with <provider>/<model> (sends: your query)".
- [ ] **Step 5: Commit** `feat: omnibox routing and search providers`

### Task 2: `reader` helper process

**Files:**
- Create: `crates/reader/{Cargo.toml,src/lib.rs,src/main.rs,src/extract.rs,src/client.rs}`
- Modify: `NOTICE`, `deny.toml` (Obscura deps), `scripts/bundle.sh` (ship `wren-reader` in `Contents/MacOS/`)
- Test: `crates/reader/tests/{extract.rs,client.rs}`

**Interfaces:**
- Binary `wren-reader` speaks newline-delimited JSON on stdin/stdout: request `{"id", "url", "timeout_ms", "render": "auto"|"never"}`, response `{"id", "ok": {"url", "final_url", "title", "byline", "published", "lang", "text", "links": [..], "needs_render": bool}}` or `{"id", "err"}`.
- `reader::Client` in the browser: spawns the binary on first use, multiplexes requests, restarts on crash, kills it after idle. `needs_render == true` (empty main text with heavy script) makes `genui` retry in a hidden Servo webview through `agent::read_page`.
- Uses Obscura with JS on (V8) by default and `render: "never"` for plain fetch + extract; `obscura-ssrf` blocks private targets before any connect.

- [ ] **Step 1: Write failing tests**: `extract_article_text` (fixture HTML), `reader_refuses_private_targets`, `client_restarts_after_crash`, `client_exits_helper_after_idle`.
- [ ] **Step 2: Run** `cargo nextest run -p reader`. Expected: FAIL.
- [ ] **Step 3: Implement.** First check Obscura builds with the rustls feature set only (`cargo tree -p reader -e features | grep -i boring` must be empty).
- [ ] **Step 4: Run** tests; measure the helper's peak RSS reading 5 news pages (`/usr/bin/time -l`), record it in the PR (spec target about 30 MB without JS).
- [ ] **Step 5: Commit** `feat(reader): headless reader helper on obscura`

### Task 3: `genui` pipeline and format interface

**Files:**
- Create: `crates/genui/{Cargo.toml,src/lib.rs,src/pipeline.rs,src/format.rs,src/openui.rs,src/prompt.rs,src/components.json}`
- Modify: `crates/store` (migration `005_genui.sql`: pages, versions, pins)
- Test: `crates/genui/tests/pipeline.rs` (fake provider, fake search, fake reader)

**Interfaces:**
- `pub trait Format { fn system_prompt(&self, components: &ComponentLibrary) -> String; fn parse_stream(&self, chunk: &str) -> Vec<Statement>; fn apply_patch(&self, program: &Program, patch: &[Statement]) -> Program; }`; `OpenUiFormat` implements it with OpenUI Lang (statements keyed by id; follow-ups return only changed statements).
- `ComponentLibrary` loaded from `components.json` (name, props schema, description): `Card`, `Table`, `Chart` (line/bar), `Comparison`, `Map` (static tiles off by default: renders a place list unless a tile provider is configured), `Form`, `SourceList`, `ImageGallery`, `Callout`, `Steps`.
- `Pipeline::run(query, ctx) -> impl Stream<Item = PipelineEvent>` with events `Planning`, `Searching{queries}`, `Reading{url}`, `Statement(Statement)`, `Sources(Vec<Source>)`, `Done{page_id}`, `Error`. Steps: model plans up to 3 queries (structured output), search top 5 each, dedupe, read top 6 with the reader (parallel 3), generate with sources numbered `[1]..[n]`, validate every statement against the library.
- Pages stored as `{id, query, program, sources, created_at}`; versions on each edit.

- [ ] **Step 1: Write failing tests**: `plans_queries_then_reads_top_results`, `every_fact_block_has_source`, `invalid_component_statement_dropped_with_error`, `follow_up_patches_only_changed_statements`, `pipeline_events_in_order`.
- [ ] **Step 2: Run** `cargo nextest run -p genui`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** tests.
- [ ] **Step 5: Commit** `feat(genui): generation pipeline with openui format`

### Task 4: `wren://gen/<id>` renderer page and tool bridge

**Files:**
- Create: `web/gen/{package.json,bun.lock,index.html,src/main.tsx,src/components/*.tsx,src/bridge.ts,build.ts}`, mise tasks `gen:build`, `test:gen`
- Modify: `crates/wren/src/{internal_pages.rs (wren:// handler),window.rs}`, `scripts/bundle.sh`
- Test: `web/gen/src/*.test.ts` (`bun test`), `crates/wren/tests/gen_page.rs`, CDP test `gen_page_renders_stream`

**Interfaces:**
- `bun run build.ts` bundles to `$CARGO_TARGET_DIR/gen-bundle/` (React + `@openuidev/react-lang` + components), embedded at build time with `include_dir!` behind a build-script check that the bundle exists (fails with "run mise run gen:build").
- `bridge.ts`: `toolProvider(name, args)` -> `fetch("chrome-extension://wren-gen/__wren/api", POST {ns:"gen", method:"tool", args:{page_id, name, args}})`; stream subscription via the same RPC returning queued statements (long-poll, 25 s).
- Tools available to generated pages (8.5): `search`, `read_url`, `webmcp:<origin>:<name>` (consequential ones through M4 policy confirm), plugin tools later (M7).
- Page chrome: query title, "Edit" follow-up box, "Pin" button, sources panel; unknown components render an inline error block.

- [ ] **Step 1: Write failing tests**: bun `renders_card_and_table`, `unknown_component_renders_error`, `script_statement_not_executed`; Rust `gen_page_served_with_csp`; CDP `gen_page_renders_stream` (fake pipeline streams 3 statements, DOM shows them in order).
- [ ] **Step 2: Run** `mise run test:gen`, `cargo nextest run -p wren --test gen_page`, `mise run test:cdp`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** all; screenshot skill of a generated comparison page (fake pipeline): cards, table, sources visible. Check Servo renders the React bundle; if a component fails in Servo, simplify it (no layout features Servo lacks) and note it in the PR.
- [ ] **Step 5: Commit** `feat: wren://gen renderer with openui and tool bridge`

### Task 5: Pin and refresh

**Files:**
- Create: `crates/genui/src/pins.rs`, `crates/wren/src/sidebar/pins.rs`
- Test: `crates/genui/tests/pins.rs`, `crates/wren/tests/pins_ui.rs`

**Interfaces:**
- `Pins::{pin(page_id, refresh_minutes: Option<u32>), unpin, list, due(now) -> Vec<PageId>, record_refresh(page_id, Result<ToolData, String>)}`; refresh re-runs every `Query(...)` binding through the tool bridge with no model call; failures keep the last data with a `refresh_failed_at` note.
- Pinned pages listed in the sidebar (M4 panel, "Pinned" tab), open in a tab, refresh on open; the scheduler is the window tick (minute granularity) until M6's scheduler takes over.

- [ ] **Step 1: Write failing tests**: `pin_stores_program_not_snapshot`, `refresh_calls_tools_not_model`, `failed_refresh_keeps_last_data`, `due_respects_minimum_15_minutes`, `pinned_list_in_sidebar`.
- [ ] **Step 2: Run** `cargo nextest run -p genui -p wren`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** tests; screenshot of the sidebar Pinned tab.
- [ ] **Step 5: Commit** `feat(genui): pin and refresh generated pages`

### Task 5a: Logged-in site tools, per-site consent, re-check

**Files:**
- Create: `crates/genui/src/site_tools.rs`, `crates/genui/src/consent.rs`, `crates/genui/src/recheck.rs`
- Modify: `crates/agent` (`read_page` gains `Session::Profile | Session::Anonymous`), `crates/wren/src/internal_pages.rs` (`wren://settings` site list), `crates/store` (migration: `site_consent`, `pin_snapshots`)
- Test: `crates/genui/tests/{site_tools.rs,consent.rs,recheck.rs}`

**Interfaces:**
- Tools for generated pages and the generator (spec 8.4a): `site.open(url) -> TabRef`, `site.read(target) -> SiteRead { url, title, text, outline, tables, forms }`, `site.act(tab, Step)` (consequential steps through M4 policy confirm). `site.read` with `Session::Profile` uses an M3 hidden webview on the user's profile; without consent it fails with `ToolError::ConsentRequired(origin)` and the page shows the one-time prompt.
- `Consent::{check(origin) -> Grant, grant(origin), revoke(origin)}`; origins whose category is banking, mail or health return `Grant::AskEachTime`.
- `Recheck::{run(pin) -> Diff, set_alert(pin, Condition)}`: stores the last tool data per pin, returns changed blocks for highlighting, fires a macOS notification when `Condition` (number threshold on a bound value, or text contains) turns true; no model call.
- Provider disclosure: every generation records which origins' text was sent, shown in the page's sources panel.

- [ ] **Step 1: Write failing tests**: `profile_read_requires_consent`, `consent_remembered_per_origin`, `sensitive_category_asks_each_time`, `anonymous_read_uses_reader_without_cookies`, `recheck_highlights_changed_blocks`, `alert_fires_once_when_condition_turns_true`, `recheck_makes_no_model_call`, `disclosure_lists_sent_origins`.
- [ ] **Step 2: Run** `cargo nextest run -p genui -p agent`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** tests; by hand: "dashboard of my three newspapers" with three logged-in news sites produces one grouped, cited page; pin it with a daily re-check; a train search on a logged-in rail site produces a comparison page whose "Book" opens the flow in a tab and stops at payment. Screenshots of both.
- [ ] **Step 5: Commit** `feat(genui): logged-in site tools, consent and re-check`

### Task 6: Milestone exit

- [ ] **Step 1:** RAM benchmark tabs10/tabs20 vs Chrome (no regression above 10% against M4); reader helper peak RSS and idle exit recorded; one generated page open adds how much.
- [ ] **Step 2:** M5 exit: with Ollama + SearXNG (documented free path) and with one keyed pair, three questions produce cited pages; a follow-up edits one; a pinned page refreshes without a model call (provider request log shows none); the newspaper dashboard and train comparison from Task 5a work on the owner's logins. Record in `docs/research/perf/<date>-m5.md`.
- [ ] **Step 3:** Roadmap status; **Commit** `chore: m5 exit and measurements`
