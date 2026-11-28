# Belobug rendering correctness ports on Sprint 6

Reference: VIctory-Belobug-AMD-a-publier ZIP, commit 4eb6cfdf9e61cb112f1f811344ec0dd309eddd6c.
The original fork URL is unavailable. The supplied snapshot's own README reports
remaining flickering trees/signs/far objects, dark rooms without RT, and late
textures. Its performance numbers and rendering correctness are not guarantees
for this RX 6800 fork.

Parent 22b3d3d allows the two title lighting passes without enabling the BVH
builder. Cumulative branches:
- d8aa28c: computed descriptor address table (027e367); fixes refusal of four
  shadow tracers as documented by upstream, including host + shader tests.
- 025fb57: retry mask-as-data failure (d1f883ec); only on the documented refusal,
  existing successful shaders take the original path.
- fab0bda: structured culling view alignment without raw twins, permanent
  sampler null slot, raw native shader parsing, shader failure listing
  (dd67ae7, 86bb00e, e70e031). Culling may drop geometry if input views are
  wrong; this fixes the alignment mechanism, not a confirmed specific screenshot.
- d3bdaf2: material DXIL validation fixes for array CalculateLOD and signature
  interpolation rows (41d3184, 0dc5d1b). Corrects invalid shaders rather than
  modifying lighting formulas.

All patches were applied with exact matching context against the previous
branch. Original tests retained, not executed on Windows here. No claim that
the user's observed missing objects are all fixed.

The root run.ps1 reproduces the Sprint 6 configuration, with path parameters,
independent RT switches, and logs. A/B: keep save, view, weather, all other
settings and shader warmup consistent. Use -RTResolve 0 -ActivateRays 0 for
pass exclusion control. -Ambient 0 isolates title lighting. BVH defaults off.
DescriptorPages defaults 64 per published main benchmark recommendation:
this baseline accepts numbers, not 'all'. Later experimental flags are not
activated. Use -More to explicitly override settings.

Do not reintroduce broad wave changes, ambient replacement, bulk texture
scheduling changes or memory changes without a matching failure. BVH fences
and bounds already exist. Dynamic BVH size/readback corruption remains open.
