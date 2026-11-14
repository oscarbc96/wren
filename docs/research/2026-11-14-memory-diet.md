# Memory diet (2026-11-14)

Tabs10 workload, release build, macOS, 16 GB. Metric: sum of
`phys_footprint` over every process of the profile (browser plus all
content processes), median of 10 samples taken after 30 s. RSS shown for
reference.

## Per-process breakdown (default profile, tabs10)

`Engine::memory_report` (Servo `create_memory_report`) output, summed
across the browser process and the ten content processes, excluding
`vsize` and `system-heap-reserved`:

| Component | Bytes |
|---|---:|
| system-heap-allocated (browser) | 122,423,920 |
| resident (browser) | 79,986,688 |
| webrender/images | 39,711,632 |
| memory-cache/public | 30,620,688 |
| webrender/display-list | 5,681,536 |
| fonts | 2,277,376 |
| tabs system-heap-allocated (sum) | 398,831,024 |
| tabs resident (sum) | 387,203,072 |
| tabs js/malloc-heap (sum) | 136,576,592 |
| tabs js/gc-heap used (sum) | 32,630,344 |
| tabs layout box-tree (sum) | 72,357,776 |

Per tab footprint varies with page complexity (roughly 42 MB to 240 MB
excluding vsize and reserved heap).

## SpiderMonkey profile comparison (tabs10)

| Profile | js_gc_profile | js_jit | RSS median (bytes) | phys_footprint median (bytes) |
|---|---|---|---:|---:|
| default | default | true | 670,564,352 | 1,184,069,552 |
| low | low | true | 1,007,648,768 | 1,323,645,016 |
| low + jit off | low | false | 340,639,744 | 1,131,771,392 |

## Reading

- The JS GC heap is small (about 33 MB used across ten tabs); most of
  the footprint is malloc in content processes (about 400 MB), layout box
  trees (72 MB), the JS malloc heap (137 MB), and WebRender images and the
  memory cache in the browser process. A GC profile cannot move much of
  that.
- The three runs differ by less than the run-to-run noise of live sites
  (ads and dynamic content change between runs): `low` measured 140 MB
  more than `default`, and `low` with the JIT off 52 MB less. None of it is
  a reliable win.
- The total is higher than the M0 figure (830 MB) because this metric
  sums every content process of the profile; M0 summed the root's process
  tree only.

## Chosen default

`memory.js_gc_profile = "default"` and `memory.js_jit = true`. The `low`
profile stays available (earlier collection, compacting on, no empty
chunks, growth factors 1.2 to 2.0) for memory pressure handling. Turning
the JIT off is not worth the page-load risk for a gain inside the noise.

## Next levers (part 2)

Ordered by the breakdown above: memory pressure response (minimize and
sleep background tabs), allocator decay for content processes (malloc is
the largest bucket), keeping GPUI, settings and the store out of
`--content-process` start, and background GC per tab if Servo allows it.

## Task 10 part 2 (steps 3, 4, 5)

### Engine minimize_memory (Servo 0.8)

`Engine::minimize_memory` added to the engine API and recorded by `FakeEngine`.
`ServoEngine` clears the network cache via `Servo::network_manager().clear_cache()`
then calls `engine::memory_relief::relief_now()` to ask the system allocator to
return free pages. Servo 0.8 exposes no embedder API for JS GC, per-webview GC, or
a global memory-pressure/minimize message to script or layout, so GC and
image/font cache purges are not available without a Servo patch.

### Memory pressure (step 3)

`crates/wren/src/memory_pressure.rs` provides `PressureSource`, `PressureLevel`,
a fake source for tests, and a macOS implementation on
`DISPATCH_SOURCE_TYPE_MEMORYPRESSURE`. Dispatch FFI is shared from
`crates/engine/src/memory_relief.rs`. When `memory.pressure_response` is true:
on Warn, `minimize_memory` then sleep the oldest background tab that may sleep;
on Critical, sleep every background tab that may sleep (never active, pinned, or
audible; Task 2 rules). The window polls the source each tick and applies it.
Test `pressure_warn_minimizes_then_sleeps` covers warn behavior with a fake
source and `FakeEngine`.

### Background GC (step 4)

Servo 0.8 has no per-webview GC API exposed to the embedder (no WebView/Servo
method and no embedder-to-constellation GC message found in
`servo-0.8.0` / `servo-constellation-0.8.0` / `servo-embedder-traits-0.8.0`).
Running a GC 30 s after a tab goes to the background is skipped; a Servo patch
can add it later.

### Content process start and allocator relief (step 5)

`wren` main already runs only `engine::content_process_main()` in
`--content-process` mode, so GPUI, settings, the store, and rustls initialization
do not run in content processes.

On macOS Servo 0.8 does not use jemalloc in this build (memory reports show
`system-heap-allocated`), so `MALLOC_CONF`/jemalloc decay tuning is not used.
`content_process_main` now starts allocator relief via
`engine::memory_relief::start_relief_thread()`: a background thread calls
`malloc_zone_pressure_relief(std::ptr::null_mut(), 0)` every 30 s, and on macOS a
dispatch memory pressure handler triggers `malloc_zone_pressure_relief` once on
warn/critical events. Set `WREN_NO_RELIEF=1` to skip periodic and event-driven
relief (used for measurement baselines).

### Measurement (tabs10, release, `parity ram`)

Footprint summed over all profile processes, median per run, three pairs
run in alternating order (live sites make single runs noisy):

| Pair | No relief (`WREN_NO_RELIEF=1`) | Relief |
|---|---:|---:|
| 1 | 1,195 MB | 1,371 MB |
| 2 | 1,351 MB | 1,193 MB |
| 3 | 1,279 MB | 1,184 MB |
| Median | 1,279 MB | 1,193 MB |

Relief saves about 86 MB (7%) at the median; the spread between runs
(about 180 MB) is larger than the effect, so treat it as a small win.
Bigger levers remain tab sleep and fewer content processes.
