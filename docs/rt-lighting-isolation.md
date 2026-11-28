# Original lighting isolation — Sprint 6

Base: 6c274de (Sprint 6 main 40d48dd plus independent pass switches).
Reference: supplied VIctory-Belobug-AMD-a-publier snapshot, 4eb6cfdf9e61cb112f1f811344ec0dd309eddd6c.
Its scripts/play.ps1 only requests VICTORY_BVH_BUILDER=1 with -RayTracing.
Its pipeline/make.rs has no named exclusion of CS_RaytracingResolve or
CS_ScreenProbes_ActivateRays. Match that eligibility, keeping independent
explicit 0 overrides in this fork.

This does NOT enable BVH building or prove valid ray intersections. It allows
existing title compute pipelines to translate and execute if they are ready.
No shader lowering, ambient fallback, resource/descriptor, frame generation,
resolution, memory, synchronization or performance changes in this sprint.

Use the same launch settings except:
    VICTORY_BVH_BUILDER='0'
    VICTORY_RT_RESOLVE='1'
    VICTORY_ACTIVATE_RAYS='1'

For the exact baseline control, set both pass values to '0'; also test them
independently. Keep VICTORY_AMBIENT identical between runs. The runtime logs
ray-tracing pass selection with requested builder and both switches; the
diagnostic missing-pipeline reason distinguishes explicit exclusions.

Source review of the supplied snapshot:
- Existing baseline already has BVH atomic fences and bounds protection.
- Upstream 027e367 handles computed acceleration-structure addresses. The
  baseline refuses such descriptors read as data. The port would touch the
  shader/host table ABI; defer until a matching failed shader is identified.
- Upstream d1f883ec retries the FullAdaptive shadow tracer only after a
  specific mask-as-data refusal. Defer until matching diagnostic.
- Upstream getDimensions non-reuse change addresses an RDNA4 compiler
  crash; its own comment says RDNA2 compiles the affected pipelines either
  way. Do not port for RX 6800 without evidence.
- Ambient magenta rejection and settling modify the fallback, not BVH
  traversal. Do not import them while isolating pass eligibility.
- Dynamic BVH size/readback corruption remains documented upstream; the
  builder is not needed for this test.

This is a controlled reproduction of pipeline eligibility, not a claim
of resolved visual defects. Windows compile and runtime not verified here.
