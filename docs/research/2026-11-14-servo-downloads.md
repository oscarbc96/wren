# Servo 0.8 download handling research

Date: 2026-11-14
Scope: Task 5 (downloads) in M1 daily driver.

## Sources inspected

- `servo-0.8.0/webview_delegate.rs`: `WebViewDelegate` exposes
  `request_navigation`, `load_web_resource`, `show_console_message`, etc.
  There is no download/start-download callback and no response-header hook.
- `servo-0.8.0/responders.rs` and embedder traits: `WebResourceLoad` /
  `WebResourceRequest` provide request interception (url, method, headers,
  destination, referrer, main-frame flag) but not the network response
  headers or body, so `Content-Disposition: attachment` cannot be observed
  there in Servo 0.8.
- `servo-net` / `servo-script` (0.8): `Content-Disposition` parsing exists
  for form/multipart bodies, not for triggering a file download delegate.

## Chosen approach

Use `request_navigation` as the detection point for document navigations:

1. If the navigation filter denies the URL, keep the existing behavior
   (emit `NavigationDenied`, do not download).
2. If the URL has a known download extension, treat it as a download.
3. Otherwise, for http(s) main-document navigations, issue a `HEAD`
   request (short timeout) off the UI thread and inspect
   `Content-Disposition` and `Content-Type`. If the response indicates an
   attachment or a non-renderable type, emit `EngineEvent::Download` and
   deny the navigation.

Known download extensions (per plan): `.zip`, `.dmg`, `.pkg`, `.pdf`,
`.csv`, `.xlsx`, `.docx`, `.mp4`, `.tar.gz`.

## Limitations / follow-ups

- The download is fetched with `reqwest` in a background thread. Cookies
  and other per-origin credentials from the Servo session are not shared
  yet, so authenticated downloads may fail. The Servo patch queue should
  add a proper download delegate and/or cookie bridging if this matters.
- HEAD-based detection adds one extra request for candidate navigations;
  failures or timeouts fall through to normal navigation.
- `Content-Disposition` filename parsing is best-effort
  (`filename*=` UTF-8 and `filename=` token/quoted-string).
