# Belobug optimization reintegration

Base: `25c9a37` (native AMD FSR 3.1 SR+FG, descriptor normalization and RDNA2 allocator fix).

## Sprint 1 — in-place constants heap

Exact patch from preserved commit `23fb3cc`, applied using unique matching hunk context; verified that the original patch reproduces the original target files. Existing FSR backend, descriptor normalization, launcher and shader translation unchanged.

Opt-in: `VICTORY_PORT_CONSTANTS=1`. Unset or 0 uses existing copied-memory path. Attempts to open the title's low 32-bit graphics heap in place through a shared section; fallback to normal copy if the device refuses or the range is unsuitable. This does not guarantee GPU-local performance: discrete GPUs may read system memory over PCIe. Existing port assumes CPU/GPU ordering for this heap; verify visual correctness, stability and native FPS, not just FG rate. Retained mapping lifetime and synchronization assumptions are inherited from the earlier port. Windows compilation and GPU testing pending.

Run the same Series S 0.50 AMD SR+FG command with `VICTORY_PORT_CONSTANTS='1'` in `-More`. Look for `renderer port 1: title memory in place`, plus periodic region/fill/copy counters. Compare with `VICTORY_PORT_CONSTANTS='0'` using the same scene and cache. Do not combine with later sprint switches until validated.

## Sprint 2 — descriptor tracking and disjoint synchronization

Reintegrates the renderer code from preserved commit `697fb42` on the working custom AMD scale baseline `67c772a`. Existing descriptor padding normalization, FSR hooks and renderer remain intact. The loader module declaration was inserted next to the existing hooks module; the unrelated old launcher logging changes are not applied.

Opt in with `VICTORY_PORT_DESCRIPTORS=1` alongside `VICTORY_PORT_CONSTANTS=1`. Patched title descriptor stores and the layer's own calls mark affected slots; the adopted view heap uses these marks rather than scanning all changed pages. The original fallback remains enabled (ask every 8 frames, inspect 16 pages per frame by default). Do not disable fallback. Check for all six patched stores, heap adoption, missed-view counters and the heap-sync timings; if hooks are incomplete or rendering regresses, turn the switch off. The original port assumes unmarked non-descriptor overwrites need not be immediately mirrored. This is an experimental source port, not a Windows build or runtime validation.
