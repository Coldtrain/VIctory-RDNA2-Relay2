# Experimental screen-space diffuse illumination

This branch is based on 26e71d0. No launcher defaults are changed. Original mode
keeps the previous ambient implementation, including explicit pass switches.

PowerShell, before starting a fresh process:
```powershell
$env:VICTORY_LIGHTING_MODE="screen_space"
$env:VICTORY_RT_RESOLVE="0"
$env:VICTORY_ACTIVATE_RAYS="0"
$env:VICTORY_BVH_BUILDER="0"
$env:VICTORY_SCREEN_GI_LEVEL="0.8"
```
The replacement uses the existing post-dispatch hooks for
CS_CacheScreenSpaceRadiance and CS_ScreenProbes_TemporalResolve. It computes a
bounded estimate using cached radiance, depth-derived normals, deterministic
stratified sampling, depth-aware reconstruction and the existing crease AO.
It overwrites the packed diffuse output rather than blending the title's
potentially corrupted traced value. The title resolve itself still runs;
other resources and passes remain the title's responsibility. The replacement
does not modify volumetric air, reflections or direct illumination. It does not
implement DXR or physically accurate GI. The original hook's resource layout,
depth/projection assumptions and command ordering still apply.

Control run: change only VICTORY_LIGHTING_MODE to original, leaving the two
pass switches and BVH builder disabled. Also set VICTORY_AMBIENT=0 and remove
VICTORY_AMBIENT_RT in both runs to avoid the legacy ambient fallback in the
control. Compare the same save, view, weather and time of day. Restart between
runs. The replacement logs replacement=true in the stand-in message if the
existing hook actually records work. No such message means the mode did not
reach the hook; a selected mode alone is not evidence of a working replacement.

The existing integration's shader corrections are included and may themselves
have rendering regressions. No Windows build or hardware validation has been
performed here. The ambient test compiles both variants on an available D3D12
host. If the known title pass cannot bind or its descriptors do not match the
expected formats, the existing hook declines to run.
