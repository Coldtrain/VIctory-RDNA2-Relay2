# VIctory RDNA2 Relay 2

An independent source-snapshot fork of the CoolFox Relay 2 build, with bounded Direct3D 12 command-allocator retention to address Radeon device resets during gameplay.

## Gameplay result

| Configuration | Reported result |
| --- | --- |
| AMD Radeon RX 6800 (RDNA 2), 4K | **~15 FPS average in gameplay** |

Reported by Coldtrain on the patched build on November 26, 2026. This is a user-observed result, not an independent benchmark. Internal rendering resolution, scene, frame-generation status, and test duration were not supplied. No universal performance or crash-free guarantee is implied.

## What was fixed

The patch targets `DXGI_ERROR_DEVICE_RESET (0x887A0007)` associated with excessive live `ID3D12CommandAllocator` objects. It retains Relay 2, thin recording, and GPU card steps while reducing allocator retention:

| Path | Change |
| --- | --- |
| `host.rs` | Shared idle prelude pool (128); landed retention capped at 4 |
| `list.rs` | Lazy host-list allocation; idle pool reduced to 256 lists / 128 MiB |
| `list/thin.rs` | Backing-list pool capped at 64; host references cleared on retirement |
| `queue/behind.rs` | Default submission depth reduced from 64 to 32 |
| `host/card_steps.rs` | Idle step pool capped at 8 |

See [the technical fix notes](docs/rdna2-allocator-fix.md) for the reported root cause, implementation details, reproduction settings and limitations. Credit for the allocator-exhaustion diagnosis and fix design goes to the contributors to [Belobug issue #329](https://github.com/Belobug/VIctory-Belobug-AMD/issues/329).

## Build and run (Windows x64)

Install Rust via rustup, MSVC C++ Build Tools and a Windows SDK. A D3D12 GPU supporting mesh shaders and shader model 6.6, an AVX2 CPU, and your own complete title dump are required. From the repository root:

```powershell
cargo build --locked --profile diag -p victory --features diagnostics
.\scripts\play.ps1 -Exe 'D:\Games\MyDump\game_scarlett_master.exe' -More @{
    VICTORY_RELAY='2'; VICTORY_LIST_POOL='1'; VICTORY_THIN='1'
    VICTORY_CARD_STEPS='2'; VICTORY_SUBMIT_DEPTH='32'
}
```

### Graphics-only regression test

This branch starts directly from the original RDNA2 Relay2 main commit. It adds
launcher controls and the existing build-checked resolution hooks from
`perf/belobug-06a-render-scale` / `perf/belobug-06b-upscaler-choice`.
No renderer, shader translator, allocator, relay, descriptor, texture or
pipeline implementation is changed. Series S uses the baseline's existing
console profile and two-plane presenter.

```powershell
.\scripts\play.ps1 -Exe 'D:\Games\MyDump\game_scarlett_master.exe' -Console series-x -Upscaler stock
.\scripts\play.ps1 -Exe 'D:\Games\MyDump\game_scarlett_master.exe' -Console series-s -Upscaler stock
.\scripts\play.ps1 -Exe 'D:\Games\MyDump\game_scarlett_master.exe' -Console series-s -Upscaler game -RenderScale 0.5
```

Series X and unmodified scale/upscaler are the default to preserve the main
baseline. Series S reports 1920x1080 by default; override the reported display
with `-More @{VICTORY_DISPLAY_MODE='3840x2160'}` if desired.
`-RenderScale 0` preserves dynamic resolution; custom scale is 0.2–1.0.
`-Upscaler stock` preserves console selection (FSR on Series S), and requires
scale 0 to avoid the experimental FSR scaling hook. `-Upscaler game` selects
the title's own temporal upscaler; auto selects it when custom scale is set.
The hooks only apply to the known 1.0.2.1 executable hash and expected bytes.
Keep all other launch overrides and cache conditions consistent when comparing.

The launcher clears most inherited `VICTORY_*` environment variables, so use `-More` for overrides. These are recommended reproduction settings, not a verified record of the exact 15 FPS run. See the upstream instructions below for other launch options, cache configuration and known limitations.

## Source and credits

Based on the supplied CoolFox source snapshot ([codec-d/cool_fox](https://github.com/codec-d/cool_fox)), with the five-file allocator patch applied. The original Git history was not included in the supplied archive; original source notices, license, upstream credits and documentation are retained. This repository is not affiliated with the game's developer or publisher. Do not add game files, game executables, or generated assets. See [asset policy](docs/asset-policy.md), including its existing BCPack table exception, and [LICENSE](LICENSE).

---

# Upstream README: VIctory on an RX 7900 XTX

## Run the codec_d Windows build

Download and extract the Windows x64 ZIP from the [private releases](https://github.com/codec-d/cool_fox/releases/latest). Open PowerShell in the extracted folder and point the launcher at your own complete Xbox dump.

```powershell
$env:VICTORY_GAME_DIR = 'D:\Games\MyDump'
.\Play-VIctory.cmd pareto-residency
```

The launcher selects Series S and the RTX-tested residency settings. It builds its shader cache locally under `%LOCALAPPDATA%\VIctory\cache-pareto`. Set `VICTORY_CACHE_DIR` to reuse an existing cache outside the checkout. Windows x64, an AVX2 CPU, and a Direct3D 12 GPU with shader model 6.6 and mesh shader support are required.

To build from this checkout, install the pinned Rust toolchain and the MSVC C++ build tools with a Windows SDK, then run `cargo build --release --locked -p victory`. The same launcher finds `target\release\victory.exe`. The sections below describe the upstream AMD fork and its separate launcher.

A fork of [VIctory](https://github.com/Marvisak/VIctory), a compatibility layer
that runs an Xbox Series title natively on Windows from files the user supplies.
This one is about **speed on AMD RDNA 3**: the world at 30 frames a second on
one PC, without giving up the picture. It is not there yet.

**This repository contains no game files, code or assets.** You need your own
copy of the game. It is not affiliated with or endorsed by the game's developer
or publisher.

## Where it stands (2026-11-25)

Tested on one machine only: Windows 11, Intel i9-12900K, 64 GB RAM, Radeon
RX 7900 XTX, 1440p, the Series X profile.

| | |
|---|---|
| Menus, videos, loading screens | 30 fps |
| Prologue, indoors | 50-75 ms a frame (13-20 fps) |
| First mission with the boat | 46-53 ms (19-22 fps) |
| Street chase with many cars and people | 100-165 ms (6-10 fps), single frames far longer |
| Outdoors on foot, the camp | 60-90 ms |
| Flying | 42-57 ms |
| Longest session so far | 41 minutes, no crash |
| Memory | 40-46 GB of private memory: **64 GB of RAM recommended** |

It started at 280 ms a frame in the same scenes two days earlier.

What is known to be wrong:

- Trees, plants and far buildings flicker outdoors in some places.
- Rooms with little direct light are too dark without `-RayTracing`.
- Textures of what just came into view appear late while the title streams.
- The first run with a new driver or a new build stutters while the driver makes
  pipelines (`victory warm` makes them ahead).
- Recording a frame in the world makes the title's own watchdog end it.

## How to run it

You need Windows 10/11 x64, a CPU with AVX2, a Direct3D 12 GPU with shader
model 6.6 and mesh shaders, Rust (the toolchain is pinned), the MSVC build
tools, and your own dump of the title.

```powershell
git clone https://github.com/avatarmanz/VIctory-7900XTX
cd VIctory-7900XTX
copy victory.example.toml victory.toml      # then set your paths, outside this folder
cargo build --locked --profile diag -p victory --features diagnostics
scripts\play.ps1 -Exe <path of your copy's executable>
```

`scripts\play.ps1 -Safe` leaves out the newest, faster ways; try it first when
something is wrong. `-RayTracing` turns the title's ray-traced indirect light on.

Before the first run, ahead of time (both can be stopped and go on later):

```powershell
target\diag\victory.exe shaders <the shader archives>   # every shader translated, minutes
target\diag\victory.exe warm --used                     # pipelines earlier runs drew with, for a new driver or build
```

Commands, debugging switches and upstream's own description:
[docs/README-fxceless.md](docs/README-fxceless.md). Every `VICTORY_...` switch is
described in a comment where it is read.

## What is different here

In short; the reasons and the measurements are in [docs/findings.md](docs/findings.md).

- **List calls are written down, not carried out, on the title's threads**
  (`VICTORY_THIN`): eight threads of ours translate them.
- **Submissions are prepared by a thread of ours and joined**
  (`VICTORY_ASYNC_SUBMIT`, `VICTORY_JOIN_PARTS`), with what can be spread over
  threads spread: refreshes, compares (AVX2), descriptor records, landings.
- **What shaders wrote comes back beside the write guards**, by the dwords that
  changed, on a thread of its own (`VICTORY_RELAY`, `VICTORY_LAND`).
- **Buffer fills that need bytes still on the card are done on the card**
  (`VICTORY_CARD_STEPS`) instead of waiting for it.
- **The title's writes are caught more cheaply**: one view opened at a fault,
  blocks that rest, places in the title's code that look before they write.
- **Textures go up on a copy queue of their own** and a slot shows one only after
  its upload ran, which also ended a device loss that came every 15 minutes or so.
- **Boats float**: textures that shaders write and the title's CPU reads go back
  to title memory (from fxceless and Geckomacabre, on by default here).
- **Frame record and replay** with per-pass times and shader variants compared
  turn by turn ([docs/replay.md](docs/replay.md)).

## Credits

This is a tree of other people's work with more on top. The upstream repository history contains original commit attribution. This imported source snapshot does not include that Git history.

- **[VIctory](https://github.com/Marvisak/VIctory)** by Marvisak and its
  contributors: the loader, the runtime services, the Direct3D 12 back end and
  the shader recompiler.
- **[fxceless/victory-fork](https://github.com/fxceless/victory-fork)**: the fork
  this one started from, and the people whose pull requests it carries.
- **[DexNom/VIctory](https://github.com/DexNom/VIctory)**: ray tracing fixes,
  cube gradients, sparse textures.
- **[Belobug/VIctory-Belobug-AMD](https://github.com/Belobug/VIctory-Belobug-AMD)**:
  the ambient light idea, pipeline workers, and work by romainhedouin.
- **[FQY55kai/ZJOU-VICTORY](https://github.com/FQY55kai/ZJOU-VICTORY)**.
- **[myokex/victory-amd](https://github.com/myokex/victory-amd)**: the guard over
  every view of a page (with Spexivus, joroyt and tightfall), and Joshua Odom's
  reads into title memory.
- **Geckomacabre** and **cxy0072019-sketch**: the texture read-back.
- **[XCL](https://github.com/ivall/xcl)** (GPL-2.0-only) was read for how the same
  title behaves under another layer; no code of it is here.
- This fork's changes were made with [Claude Code](https://claude.com/claude-code).

## License

GPL-3.0-or-later, as VIctory. See [LICENSE](LICENSE); the one file that is not
covered is named in [docs/asset-policy.md](docs/asset-policy.md).
