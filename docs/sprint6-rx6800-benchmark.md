# RX 6800 performance report — Sprint 6 (2026-11-27)

Tested source: [Sprint 6, `1c9cf08`](https://github.com/Coldtrain/VIctory-RDNA2-Relay2/commit/1c9cf089158857b52b5fcf1eb8f10843b0bcb09c). This is a maintainer-supplied Windows test, not an independent benchmark. The source checkout name appears in the log; the executable reports build "unknown", so exact commit provenance is maintainer-reported, not embedded in the binary.

## Test setup

- AMD Radeon RX 6800 (RDNA2), Ryzen 7 9800X3D (maintainer-reported hardware).
- Xbox Series S profile; 576 × 324 internal rendering (0.30 scale) to 1920 × 1080 output, confirmed by the AMD upscale context.
- Native AMD FSR 3.1 super resolution and frame generation through the FidelityFX DX12 SDK, not driver AFMF.
- 60 Hz title timing patch: three of three reads patched, 16.667 ms host interval.
- Relay 2 performance baseline, asynchronous submission, eight thin recording threads and eight texture upload workers. See [optimization notes](belobug-optimizations.md).

## Results

| Metric | Rendered FPS | Presented FPS (including FG) |
| --- | ---: | ---: |
| Arithmetic mean | 24.10 | 47.99 |
| Median | 23.61 | 46.71 |
| 10th percentile of samples | 16.78 | 33.58 |
| Peak individual sample | 34.84 | 70.68 |

259 positive-presentation samples with status ON. 44 reached at least 60 presented FPS; 16 were below 30. These are meter-window statistics, not individual-frame 1% lows or time-weighted throughput. An additional first ON transition measured 0.39 rendered FPS / 0 presented FPS; including it gives 260 ON samples with means of 24.01 / 47.80 FPS. 83 WAITING samples were excluded from both ON aggregates.

The log spans 23:34:43–23:40:37 UTC (about 5 min 54 s). ON samples span about 4 min 27 s including the transition. It ends with window closed and title suspended/ending, with no recorded device removal or host GPU failure. This is not a guarantee of long-session stability. Generated presentation estimates are not physical display scanout and do not increase simulation or native rendering throughput.

Source: maintainer log `fsr-framegen-20261127-183442(1).log` (SHA-256 `81c9801042364d1cb06b69873ab8955df79422efb3350dc473864f85e221fecb`). Parse ANSI-stripped UTF-16 text, match the FSR FG presentation meter's native_fps, present_fps and status fields across line wraps, then calculate statistics of ON samples with positive present_fps. Raw log is not published because it contains local paths and verbose runtime data.

## Historical comparison

The earlier README's `b752c46` run at 960 × 540 internal / 1080p output reported 17.4 rendered / 34.7 presented FPS averaged across 959 ON meter samples. Sprint 6 is faster in this session, but the internal resolution, timing and scene mix differ, so this is **not** an isolated AMD residency or renderer optimization gain. The precise contribution of Sprint 6 requires a matched A/B test.

Known limitations: incomplete lighting and material rendering, scene-dependent FPS, and experimental low-resolution scaling/temporal artifacts. This is the performance baseline before the subsequent rendering integration branches, not a visual-correctness release.
