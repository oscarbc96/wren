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
