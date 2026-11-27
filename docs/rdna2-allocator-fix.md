# RDNA2 command allocator exhaustion

## Evidence and diagnosis

The [upstream issue #329](https://github.com/Belobug/VIctory-Belobug-AMD/issues/329) describes device resets (`0x887A0007`) with empty DRED breadcrumbs and no active GPU work reported. [MamaLeVrai's debug-layer report](https://github.com/Belobug/VIctory-Belobug-AMD/issues/329#issuecomment-7203306341) recorded D3D12 message 834 (kernel memory failure) and 1,997 live command allocators on an RX 6950 XT. The [follow-up fix report](https://github.com/Belobug/VIctory-Belobug-AMD/issues/329#issuecomment-7204076013) recorded 2,045 live allocators on an RX 6700 XT and attributed the failures to allocator exhaustion near an observed 2,048-object threshold. This threshold is a reported driver behavior, not a documented universal D3D12 limit. Slow compute PSO creation can coincide with the failure without being its root cause.

## Implementation in this source snapshot

- `crates/victory-gpu/src/host.rs`: replace permanent per-HostList prelude ownership with a mutex-protected optional prelude; reuse completed preludes through a shared idle pool capped at 128. Return the prelude on safe list reset. Clamp landed retention to `bury_lag().min(4)`.
- `crates/victory-gpu/src/list.rs`: reduce `IDLE_MOST` to `(256, 128 << 20)`, defer HostList acquisition after detach and for backing lists, and acquire when recording requires it.
- `crates/victory-gpu/src/list/thin.rs`: cap `BACKS` at 64 and clear retired backing-list host and synchronization references before pooling.
- `crates/victory-gpu/src/queue/behind.rs`: reduce default `VICTORY_SUBMIT_DEPTH` to 32; explicit overrides still work.
- `crates/victory-gpu/src/host/card_steps.rs`: cap the completed step-list pool at 8. This CoolFox snapshot returns completed step lists directly to its pool and has no separate `cool_down` function, so the upstream `bury_lag().min(16)` cooldown change is not applicable here.

These limits bound idle retention, not the total number of simultaneously active D3D12 command allocators. The patch does not claim to enforce a global device-wide ceiling. It does not change shader translation or renderer behavior intentionally.

## Testing

Use the same executable, scene, driver, cache state, console profile, resolution and frame-generation configuration for comparison. Run with Relay 2, list pooling, thin recording, card steps 2 and depth 32. Compare frame times and stability, then use a diagnostic build with `VICTORY_D3D12_DEBUG=1` passed via the launcher's `-More` map if the debug layer is installed. Record any message 834, DRED output, and live allocator counts if available. For an A/B workaround test, set `VICTORY_LIST_POOL=0` instead. Do not treat the absence of a reset in a short run as proof of a permanent fix.

The local owner reports approximately 15 FPS average at 4K on RX 6800 with this patched build (2026-11-26). Exact render scale, benchmark duration and other configuration details are unrecorded. The upstream contributor's longer stability tests were conducted on a different build and GPU, not this source snapshot.

## Attribution

Diagnosis and proposed five-part fix: contributors to Belobug/VIctory-Belobug-AMD issue #329, including MamaLeVrai's debug capture and the linked follow-up report. This snapshot adapts that fix to CoolFox Relay 2. Existing upstream credits and licensing remain in the root README and LICENSE.
