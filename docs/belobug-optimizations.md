# Belobug optimization reintegration

Base: `25c9a37` (native AMD FSR 3.1 SR+FG, descriptor normalization and RDNA2 allocator fix).

## Sprint 1 — in-place constants heap

Exact patch from preserved commit `23fb3cc`, applied using unique matching hunk context; verified that the original patch reproduces the original target files. Existing FSR backend, descriptor normalization, launcher and shader translation unchanged.

Opt-in: `VICTORY_PORT_CONSTANTS=1`. Unset or 0 uses existing copied-memory path. Attempts to open the title's low 32-bit graphics heap in place through a shared section; fallback to normal copy if the device refuses or the range is unsuitable. This does not guarantee GPU-local performance: discrete GPUs may read system memory over PCIe. Existing port assumes CPU/GPU ordering for this heap; verify visual correctness, stability and native FPS, not just FG rate. Retained mapping lifetime and synchronization assumptions are inherited from the earlier port. Windows compilation and GPU testing pending.

Run the same Series S 0.50 AMD SR+FG command with `VICTORY_PORT_CONSTANTS='1'` in `-More`. Look for `renderer port 1: title memory in place`, plus periodic region/fill/copy counters. Compare with `VICTORY_PORT_CONSTANTS='0'` using the same scene and cache. Do not combine with later sprint switches until validated.

## Sprint 2 — descriptor tracking and disjoint synchronization

Reintegrates the renderer code from preserved commit `697fb42` on the working custom AMD scale baseline `67c772a`. Existing descriptor padding normalization, FSR hooks and renderer remain intact. The loader module declaration was inserted next to the existing hooks module; the unrelated old launcher logging changes are not applied.

Opt in with `VICTORY_PORT_DESCRIPTORS=1` alongside `VICTORY_PORT_CONSTANTS=1`. Patched title descriptor stores and the layer's own calls mark affected slots; the adopted view heap uses these marks rather than scanning all changed pages. The original fallback remains enabled (ask every 8 frames, inspect 16 pages per frame by default). Do not disable fallback. Check for all six patched stores, heap adoption, missed-view counters and the heap-sync timings; if hooks are incomplete or rendering regresses, turn the switch off. The original port assumes unmarked non-descriptor overwrites need not be immediately mirrored. This is an experimental source port, not a Windows build or runtime validation.

## Sprint 3 — raw CBV reads (upstream renderer port 5)

Reintegrates preserved commit `7e1178f` exactly on Sprint 2, without the unrelated texture upload worker changes. Keeps the earlier correctness safeguards: root CBVs remain conventional, dynamic direct placement requires a valid conventional CBV as well as a raw twin, noneligible buffers use the scratch copy, and shader cache keys are separate when raw CBV is enabled. Existing FSR implementation and scale hooks are unchanged.

First test `VICTORY_PORT_CBV=1`, `VICTORY_PORT_CBV_PLACE=0`, `VICTORY_PORT_CBV_RELAY2=0` to validate raw shader reads independently of direct placement. This alone does not remove constant-buffer copies. Then, only after visual validation, test `VICTORY_PORT_CBV_PLACE=1` and `VICTORY_PORT_CBV_RELAY2=1` with Relay 2. The latter explicitly opts into experimental shared-memory lifetime assumptions; compare animation, lighting, textures, native frame timings and the logged in-place/copy counts. Preserve `VICTORY_PORT_CONSTANTS=1`, `VICTORY_PORT_DESCRIPTORS=1`, `VICTORY_PORT_DESCRIPTORS_EVERY=1`, `VICTORY_PORT_DESCRIPTORS_CHECK=64` in both tests. New shader translation/cache may cause first-run compilation stutter; compare warmed runs. Disable all three CBV switches to recover previous path. Original unit/GPU test ported; Windows compilation and GPU execution not verified here.

## Sprint 4 — parallel memory guards (renderer port 6)

Exact preserved source patch from `9c607ee` on Sprint 3. Enable `VICTORY_PORT_GUARD=1` to reduce global guard lock contention and parallelize refresh planning. Existing FSR, scaling, CBV, descriptor fallback and allocator behavior preserved. Original tests included; Windows compilation and runtime validation pending. Compare guard wait, protection changes, refresh planning and native FPS. Shared-memory synchronization remains experimental; disable the flag if visual regressions appear.

## Optional 60 FPS timing patch

Ported timing code and tests from Coldtrain/VIctory-7900XTX commit `a3e4025e5152a9e97a64afa0161fe2d194520162`. Enable with `VICTORY_FRAME_INTERVAL_MS=16.667`, `VICTORY_TITLE_FPS=60`, optionally `VICTORY_TIMER_LOG=1`. The title hash and all three instruction sites must match or execution is refused. Two title FPS loads and minimum simulation step are updated with the layer's event clock; maximum simulation step is unchanged. This raises the cap, not GPU throughput, and does not decouple simulation and render threads. Verify game speed and pacing below 30 FPS and above it. Unset both timing switches to restore stock timing. Windows compilation and gameplay validation pending.
