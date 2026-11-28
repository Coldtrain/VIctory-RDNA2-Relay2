# Geometry correctness sprint 6

Base b66ccf6, preserves the prior five rendering sprints and performance settings.

## Evidence from supplied Belobug snapshot (4eb6cfd)

- crates/victory-gpu/src/shadow.rs: send_changed is OFF by default. Its
  comment documents run tile1-0, where partial refreshes lost light and
  geometry exploded; bis1-0 differed by this switch and rendered correctly.
  Our baseline already has this fix (upstream 6ae06963). The launcher
  explicitly sets VICTORY_SEND_CHANGED=0, preventing accidental inheritance.
- emit/lower.rs uses the correct nine-bit mesh corner indices and ignores
  edge flags; emit/mod.rs includes the flagged triangle regression tests.
  The baseline already includes the same behavior (77b82c9).
- The raw UAV globally coherent flag used by ped blend-shape skinning
  (875a63a4) and its size_tests.rs test are already in the baseline.
- Vertex/index view bounds and mesh stage input parsing fixes are already
  in the baseline.

## Separate source-confirmed ordering defect

XaviFortes/VIctory-RDNA 631400f, 3e04ca0, 15c3d0e, as previously adapted
in this repo's 7c93b5d4 (not in the supplied Belobug snapshot).
The documented repro is disappearing palms and trees, not specifically
stretched NPCs: the in-place constants heap also holds indirect records;
batch/window pre-clamping can read them before preceding culling dispatches
write them. The existing implementation on b66ccf6 uses the affected
batch/window paths. Exact context-checked port of 7c93b5d4 limits the change
to in-place arguments/counts, retains all other batches/windows, and logs
affected indirect executions. Enable with VICTORY_PORT_CONSTANTS_ORDER=1 and
VICTORY_PORT_CONSTANTS_ORDER_ALONE=1. No lighting or shader changes.

## Controlled tests

Run the same save/view and let pipelines settle, with run.ps1. Compare
-GeometryOrdering 1 vs 0 (restart). Both retain SEND_CHANGED=0 and the
previous lighting/FSR/relay options. The log should contain "indirect
executions with arguments or count in place". If no records are affected,
this change cannot explain the symptom. If geometry still stretches,
test -More @{VICTORY_PORT_CBV_PLACE='0'} separately (raw shaders retained,
copied constant buffers). This tests a possible stale transform path; not
a demonstrated fix. A further independent test is PORT_CONSTANTS=0,
which sacrifices performance, to isolate the placed 174 MB skinning heap.
Do not combine A/B toggles when diagnosing the cause.

The old source notes specifically establish the partial-refresh regression
and the separate stale-indirect-record defect. They do not establish that
the user's NPC stretching has the same cause. Windows build/GPU execution
not verified here.
