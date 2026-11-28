# Sprint 6 + Belobug rendering fixes. Run from the repo root.
# Example: .\run.ps1 -Exe 'F:\Games\GTAVI\game_scarlett_master.exe' -FsrDll 'F:\Games\FSR\amd_fidelityfx_dx12.dll'
param(
    [string]$Exe = 'F:\Games\GTAVI\game_scarlett_master.exe',
    [string]$FsrDll = 'F:\Games\FSR\amd_fidelityfx_dx12.dll',
    [ValidateRange(0,1)][int]$RTResolve = 1,
    [ValidateRange(0,1)][int]$ActivateRays = 1,
    [ValidateRange(0,1)][int]$BvhBuilder = 0,
    [ValidateRange(0,4)][double]$Ambient = 0.8,
    [ValidateRange(0,2147483647)][int]$DescriptorPages = 64,
    [ValidateRange(0,1)][int]$GeometryOrdering = 1,
    [hashtable]$More = @{},
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
$Exe = (Resolve-Path $Exe).Path
$FsrDll = (Resolve-Path $FsrDll).Path
Push-Location $PSScriptRoot
try {
    if (-not $SkipBuild) {
        cargo build --locked --profile diag -p victory --features diagnostics
        if ($LASTEXITCODE -ne 0) { throw "Build failed: $LASTEXITCODE" }
    }
    New-Item -ItemType Directory -Path '.\logs' -Force | Out-Null
    $settings = @{
        VICTORY_DISPLAY_MODE='1920x1080'
        VICTORY_OWN_UPSCALER='1'
        VICTORY_PLANES='1'
        VICTORY_FRAME_INTERVAL_MS='16.667'
        VICTORY_TITLE_FPS='60'
        VICTORY_FSR_SDK_DLL=$FsrDll
        VICTORY_FSR_FRAMEGEN='1'
        VICTORY_AFMF='0'
        VICTORY_PORT_AMD_TEXTURE_BUDGET='1'
        VICTORY_TEXTURE_MAX='0'
        VICTORY_PORT_CONSTANTS='1'
        # Preserve complete refreshes (Belobug 6ae06963: partial refreshes exploded geometry).
        VICTORY_SEND_CHANGED='0'
        # Clamp only in-place indirect records at their point of execution.
        VICTORY_PORT_CONSTANTS_ORDER=[string]$GeometryOrdering
        VICTORY_PORT_CONSTANTS_ORDER_ALONE='1'
        VICTORY_PORT_DESCRIPTORS='1'
        VICTORY_PORT_DESCRIPTORS_CHECK=[string]$DescriptorPages
        VICTORY_PORT_DESCRIPTORS_EVERY='1'
        VICTORY_PORT_CBV='1'
        VICTORY_PORT_CBV_PLACE='1'
        VICTORY_PORT_CBV_RELAY2='1'
        VICTORY_PORT_GUARD='1'
        VICTORY_PORT_GUARD_PLAN='1'
        VICTORY_RELAY='2'
        VICTORY_LIST_POOL='1'
        VICTORY_SUBMIT_DEPTH='32'
        VICTORY_ASYNC_SUBMIT='1'
        VICTORY_WIDE_BACK='1'
        VICTORY_RELAY_COPY='1'
        VICTORY_CARD_STEPS='2'
        VICTORY_CARD_REHASH_RUNS='1'
        VICTORY_CARD_AWAKE='1'
        VICTORY_THIN='1'
        VICTORY_THIN_THREADS='8'
        VICTORY_UPLOAD_AHEAD='1'
        VICTORY_UPLOAD_WORKERS='8'
        VICTORY_JOIN_PARTS='8'
        VICTORY_JOIN_COPIES='1'
        VICTORY_MAP_THREADS='1'
        VICTORY_INDIRECT_WINDOW='1'
        VICTORY_INDIRECT_BATCH='2'
        VICTORY_TEXTURE_NO_WAIT='1'
        VICTORY_AMBIENT=$Ambient.ToString('G',[Globalization.CultureInfo]::InvariantCulture)
        VICTORY_BVH_BUILDER=[string]$BvhBuilder
        VICTORY_SHOW_AFTER='1'
        VICTORY_RT_RESOLVE=[string]$RTResolve
        VICTORY_ACTIVATE_RAYS=[string]$ActivateRays
        VICTORY_TITLE_PRIORITIES='0'
        VICTORY_D3D12_DRED='light'
        VICTORY_SKIP_PIPELINES='none'
    }
    # The later experimental branch switches are intentionally not active:
    # VICTORY_PRECISE_PACING, VICTORY_TIMER_LOG, VICTORY_PORT_AMD_PROBE_FILTER,
    # VICTORY_PORT_JOURNAL, VICTORY_TEXTURE_PRELUDE, VICTORY_AMBIENT_SETTLE,
    # VICTORY_AMBIENT_RT, VICTORY_LIGHTING_MODE, VICTORY_SCREEN_GI_LEVEL.
    # This branch parses the descriptor fallback budget as a number, not 'all'.
    foreach ($k in $More.Keys) { $settings[$k] = $More[$k] }
    $log = ".\logs\fsr-framegen-$(Get-Date -Format 'yyyyMMdd-HHmmss').log"
    .\scripts\play.ps1 -Exe $Exe -Console series-s -RenderScale 0.3 -Upscaler amd -More $settings 2>&1 | Tee-Object -FilePath $log
} finally { Pop-Location }
