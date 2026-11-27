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

PowerShell (from repository root, using the baseline's existing launcher):

    cargo build --locked --profile diag -p victory --features diagnostics
    .\scripts\play.ps1 -Exe "F:\Games\GTAVI\game_scarlett_master.exe" -Console series-s -Upscaler stock -More @{
        VICTORY_FSR_FRAMEGEN = "1"
        VICTORY_FSR_SDK_DLL = "C:\SDK\FidelityFX-1.1.4\bin\amd_fidelityfx_dx12.dll"
        VICTORY_AFMF = "0"
    }

The launcher clears other VICTORY_* environment variables, so put SDK settings
in -More, not in preceding environment assignments. Leave driver AFMF off in
Adrenalin and do not use a custom render scale for this first test.

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

## Presentation meter

The window caption shows native/present FPS, the AMD FG state and estimated
extra present FPS. The DXGI count is sampled once per second from the real
swapchain (the SDK forwards GetLastPresentCount). Native FPS counts successful
calls to the replacement swapchain; the excess estimates generated presents.
The asynchronous SDK presenter can cross sample boundaries; the estimate is
not an exact per-frame classification or proof of scanout. The log also shows
successful interpolation dispatches per second separately. A failed or first
sample displays -- rather than assuming 2x. Without an AMD swapchain, FG OFF
is shown. No waits, changes to the upscaler, or synthetic counters are added.

## Own temporal upscaler capture (diagnostic, not FG support yet)

The game's own upscaler (VICTORY_OWN_UPSCALER=1) bypasses the FidelityFX
dispatch. The SDK still loads, but interpolation waits for inputs. Do not
force the stock path or reuse old depth/motion. To identify the actual own
upscaler pass, capture its resource binding and shader hashes using the
existing diagnostic operation listing:

    New-Item -ItemType Directory -Force .\fsr-own-probe | Out-Null
    .\scripts\play.ps1 -Exe "F:\Games\GTAVI\game_scarlett_master.exe" -Console series-s -RenderScale 0.30 -Upscaler game -More @{
        VICTORY_FSR_FRAMEGEN = '0'
        VICTORY_FSR_CAPTURE = '0'
        VICTORY_AFMF = '0'
        VICTORY_CAPTURE = (Resolve-Path .\fsr-own-probe).Path
        VICTORY_CAPTURE_WORLD = '1'
        VICTORY_CAPTURE_WORLD_ALSO = '120'
    }

Keep the resulting frame-*-ops.txt listings (shader hashes, roots, texture
formats and sizes). The capture adds diagnostic overhead; do not benchmark
with it. A second stock capture at the same scene can help compare the
depth and motion resources. No own-upscaler inputs are assumed until the
shader binding and camera constants are verified.

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

## Experimental synchronized stock sizing

Based on Foxhound's native_1440p::prepare_upscaler (from DexNom, GPL-3.0-or-later).
The opt-in VICTORY_FSR_STOCK_SIZING=1 hook selects renderer 7, writes the
quality mode, redirects sizing and matches the temporal scale, after verifying
all original hashes and constants. It does not apply the old forced maximum
or vtable patch. play.ps1 -Console series-s -Upscaler stock -RenderScale 1
(or 0.666667, 0.58, 0.5, 0.333333) enables it; scale 0 remains unmodified.
The Xbox FidelityFX upscaler is documented by Foxhound as working at stock
Series S sizes; acceptance of smaller custom sizes has NOT been established.
First test with VICTORY_FSR_FRAMEGEN=0, then repeat the exact same scene
with VICTORY_FSR_FRAMEGEN=1 and VICTORY_FSR_SDK_DLL pointing to the existing
SDK. Look for 'stock sizing: renderer 7, quality mode and temporal scale
synchronized' and actual render dimensions in 'submitted FidelityFX inputs'.
Use scale 0 or the original branch if custom sizes fail. The game temporal
renderer remains unchanged, and its independent FG capture is not implemented.
