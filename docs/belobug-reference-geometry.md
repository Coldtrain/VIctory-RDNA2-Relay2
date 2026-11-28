# Belobug reference geometry configuration

Base b66ccf6 (Sprint 5). This is a separate diagnostic branch, not a merge of
the XaviFortes geometry-ordering experiment 027b638.

Reference: uploaded VIctory-Belobug-AMD-a-publier ZIP (4eb6cfdf9e61cb112f1f811344ec0dd309eddd6c).

Source comparison:
- Mesh primitive corner indices (nine bits, edge flags removed), mesh stage
  output parsing, vertex/index view bounds, and globally coherent raw UAV
  loads for ped blend shapes are already implemented here.
- The reference's shadow.rs disables SEND_CHANGED by default after a
  documented exploded-geometry regression (tile1-0 vs bis1-0). Our source
  already does that too. Both modes set SEND_CHANGED=0 explicitly.
- The reference scripts/play.ps1 does not enable renderer ports 1, 2, 5,
  6 or 7 by default. Its README describes them as opt-in. Our launcher
  forced them on, including custom CBV Relay 2 placement overrides. Port 1
  maps the skinning output/constant heap in place; port 5 changes constant
  reads. To isolate the geometry path, default run.ps1 disables these
  switches but retains our native FSR, Series S, scale, timing, lighting,
  Relay 2 and other existing settings. This is not an exact whole-build
  reproduction, only a reference geometry-memory configuration.
- The reference's benchmark explicitly enabled some ports; the user's
  actual working reference run settings are not known.
- No runtime renderer code was copied or changed on this branch because
  the geometry routines reviewed were already present. No claim that
  these switches fix NPC stretching.

Run:
    .\run.ps1
Compare with same executable and scene:
    .\run.ps1 -SkipBuild -PerformancePorts
If only the reference profile works, reenable each port separately via
-More, starting with PORT_CONSTANTS and PORT_CBV separately. Avoid enabling
CBV placement on Relay 2 until the copied path is visually verified.
If both fail, the difference is not isolated to these flags; inspect
specific mesh shader failures, geometry inputs and buffer contents.
Windows build and visual validation remain pending.
