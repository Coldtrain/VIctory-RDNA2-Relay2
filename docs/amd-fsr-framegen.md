# AMD FSR 3.1 frame generation (experimental RDNA2 test)

This branch adapts the FidelityFX import and ordered input capture architecture of
[readmet3xt-lab/foxhound](https://github.com/readmet3xt-lab/foxhound/blob/main/docs/dlss.md)
without NVIDIA Streamline, DLSS, or a driver interpolation shim. The native
backend uses the **FidelityFX SDK v1.1.4** DX12 frame generation API (FSR 3.1.4)
and its interpolation swapchain. The title's existing upscale is never replaced.

## Scope and setup

First target: Series S, SDR 1920x1080, ordered submissions (THIN or ASYNC_SUBMIT).
Series X does not supply FidelityFX upscale inputs by default; its shipping
upscaler has fixed Series S tables. No Series X image patch, 4K upscaling
replacement, or game-clock change is included. Do not enable AFMF simultaneously.

Obtain the official AMD FidelityFX SDK v1.1.4 from
https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK/releases/tag/v1.1.4
and extract/build its Windows DX12 API DLL, amd_fidelityfx_dx12.dll. No SDK
binaries are included. Use the pinned SDK version, not the newer split FSR 4
loader. Point VICTORY_FSR_SDK_DLL to the full absolute DLL path.

PowerShell (from repository root):

    cargo build --locked --profile diag -p victory --features diagnostics
    $env:VICTORY_FSR_FRAMEGEN = "1"
    $env:VICTORY_FSR_SDK_DLL = "C:\SDK\FidelityFX-1.1.4\bin\amd_fidelityfx_dx12.dll"
    $env:VICTORY_THIN = "1"
    $env:VICTORY_AFMF = "0"
    .\target\diag\victory.exe run "F:\Games\GTAVI\game_scarlett_master.exe" --execute --console series-s

Keep other known-good environment switches; do not force new relay settings.
If the title starts without FG, search the log for "FSR FG". On a missing SDK,
unsupported configuration or failed native swapchain creation, the normal DXGI
chain is used. Missing fresh inputs or a reset disables interpolation on the
AMD swapchain. A driver/device error can still terminate the run.

For capture only (no AMD SDK or swapchain changes), set VICTORY_FSR_CAPTURE=1
and VICTORY_FSR_FRAMEGEN=0. The log reports submitted texture formats, sizes,
camera and motion scale. Disable both switches for the unchanged present path.

Expected log stages (not measured on a Windows GPU in this development session):

* FSR FG: FidelityFX import intercepted
* FSR FG: native AMD interpolation swapchain created
* FSR FG: submitted FidelityFX inputs
* FSR FG: native AMD frame context created
* FSR FG: enabled
* FSR FG: prepared frame
* FSR FG: SDK interpolation dispatch completed

The last message means the AMD callback recorded interpolation work successfully,
not proof a generated frame was displayed. Use PresentMon or GPU overlay to
compare displayed FPS vs VIctory's real FPS. At a low base frame rate expect
visible artifacts and latency; AMD recommends a much higher base rate. Frame
generation does not speed up simulation or real rendering. Disable overlays
which hook DXGI if they interfere with the replacement swapchain.

## Implementation and limitations

Capture is held by the host command list (including thin-list interpreter),
published only at submission and consumed once per host present. Abandoned
thin-list captures are dropped. Context destruction invalidates queued inputs.
The present list records SDK Prepare using native host depth/motion textures
in tracked compute-read states and restores their previous state. The SDK
swapchain calls ffxDispatch for generated frames, on its own callback; the
initial test does not request async compute. The existing Series S scene plane
is passed as HUD-less colour when its format, state and size are compatible;
otherwise the interpolation input includes the UI. The camera basis is a
placeholder (as in the upstream capture, the title does not supply matrices);
the actual vectors, jitter, depth range and frame time are supplied. Frame
context sizing is fixed for this first test; mismatched frames disable FG.
Before resize, disable and wait for SDK presents. Contexts and module remain
resident for the lifetime of the singleton host (no title shutdown hook).
The Windows build and GPU execution must be validated on the target system.

## Attribution

Foxhound (GPL-3.0), including its attribution to DexNom/VIctory, for the title
FidelityFX ABI and interception architecture. Host FFX ABI definitions in
native.rs are adapted from the SDK v1.1.4 headers:
https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK/tree/v1.1.4/ffx-api/include/ffx_api

Copyright (C) 2024 Advanced Micro Devices, Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
