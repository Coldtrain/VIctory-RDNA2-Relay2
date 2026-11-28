cargo build --locked --profile diag -p victory --features diagnostics
if ($LASTEXITCODE -ne 0) { pause; return }

New-Item -ItemType Directory -Path ".\logs" -Force | Out-Null

.\scripts\play.ps1 -Exe "F:\Games\GTAVI\game_scarlett_master.exe" -Console series-s -RenderScale 0.50 -Upscaler amd -More @{

    # Resolution and upscaling
    VICTORY_DISPLAY_MODE='1920x1080'
    #VICTORY_RENDER_SCALE='0.5'
    VICTORY_OWN_UPSCALER='1'
    VICTORY_PLANES='1'

    # AMD FSR
    VICTORY_FSR_SDK_DLL='F:\Games\FSR\amd_fidelityfx_dx12.dll'
    VICTORY_FSR_FRAMEGEN='1'
    VICTORY_AFMF='0'

    # AMD rendering fixes
    VICTORY_PORT_AMD_PROBE_FILTER='1'
    VICTORY_PORT_AMD_TEXTURE_BUDGET='1'
    VICTORY_TEXTURE_MAX='512'

    # Belobug renderer optimizations
    # For the reference geometry-memory A/B test, set these seven ports to '0'.
    # The values below preserve the supplied performance configuration.
    VICTORY_PORT_CONSTANTS='1'
    VICTORY_PORT_DESCRIPTORS='1'
    VICTORY_PORT_CBV='1'
    VICTORY_PORT_CBV_PLACE='1'
    VICTORY_PORT_CBV_RELAY2='1'
    VICTORY_PORT_GUARD='1'
    VICTORY_PORT_GUARD_PLAN='1'

    # Relay 2 and GPU submission
    VICTORY_RELAY='2'
    VICTORY_LIST_POOL='1'
    VICTORY_SUBMIT_DEPTH='32'
    VICTORY_ASYNC_SUBMIT='1'
    VICTORY_WIDE_BACK='1'
    VICTORY_RELAY_COPY='1'
    VICTORY_CARD_STEPS='2'
    VICTORY_CARD_REHASH_RUNS='1'
    VICTORY_CARD_AWAKE='1'

    # Parallel processing
    VICTORY_THIN='1'
    VICTORY_THIN_THREADS='8'
    VICTORY_UPLOAD_AHEAD='1'
    VICTORY_UPLOAD_WORKERS='8'
    VICTORY_JOIN_PARTS='8'
    VICTORY_JOIN_COPIES='1'
    VICTORY_MAP_THREADS='1'

    # Indirect rendering
    VICTORY_INDIRECT_WINDOW='1'
    VICTORY_INDIRECT_BATCH='2'

    # Texture streaming
    VICTORY_TEXTURE_NO_WAIT='1'

    # Geometry correctness
    VICTORY_SEND_CHANGED='0'

    # Lighting and presentation
    VICTORY_AMBIENT='0.8'
    VICTORY_BVH_BUILDER='0'
    VICTORY_SHOW_AFTER='1'
    VICTORY_RT_RESOLVE='1'
    VICTORY_ACTIVATE_RAYS='1'

    # Diagnostics
    VICTORY_TITLE_PRIORITIES='0'
    VICTORY_D3D12_DRED='light'
    VICTORY_SKIP_PIPELINES='none'
} 2>&1 | Tee-Object -FilePath ".\logs\fsr-framegen-$(Get-Date -Format 'yyyyMMdd-HHmmss').log"
pause
