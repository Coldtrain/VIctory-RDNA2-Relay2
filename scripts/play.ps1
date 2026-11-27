# Starts the title with the settings this fork is played with (README.md, "How to run it").
#   scripts\play.ps1 -Exe <path of your own copy's executable>
# -Console      series-x (default) or series-s (1080p reported by default)
# -RenderScale  0 (stock) or 0.2..1.0 (internal scale)
# -Upscaler     auto (default), game (title temporal), stock (console default), amd (native FSR)
# -AFMF         request borderless fullscreen for driver AFMF (enable per-app in Adrenalin)
# -Safe         without the newer, faster ways (the first thing to try when something is wrong)
# -RayTracing   the title's own ray-traced indirect light: lit interiors, about a third slower
# -Build        target\diag (the default, the build all measurements are of) or target\release
# -More         other settings: -More @{VICTORY_PANEL='1'}
# Run it from a normal window, not as administrator: the driver then keeps its shader cache elsewhere.
param(
    [Parameter(Mandatory)][string]$Exe,
    [ValidateSet('series-x', 'series-s')][string]$Console = 'series-x',
    [ValidateRange(0.0, 1.0)][double]$RenderScale = 0,
    [ValidateSet('auto', 'game', 'stock', 'amd')][string]$Upscaler = 'auto',
    [switch]$AFMF,
    [switch]$Safe,
    [switch]$RayTracing,
    [ValidateSet('diag', 'release')][string]$Build = 'diag',
    [hashtable]$More = @{}
)
$settings = @{
    VICTORY_TEXTURE_NO_WAIT = '1'; VICTORY_RELAY = '2'; VICTORY_WIDE_BACK = '1'; VICTORY_CARD_REHASH_RUNS = '1'
    VICTORY_JOIN_COPIES = '1'; VICTORY_INDIRECT_WINDOW = '1'; VICTORY_RELAY_COPY = '1'; VICTORY_CARD_STEPS = '2'
    VICTORY_INDIRECT_BATCH = '2'; VICTORY_MAP_THREADS = '1'; VICTORY_TITLE_PRIORITIES = '0'; VICTORY_CARD_AWAKE = '1'
    VICTORY_SHOW_AFTER = '1'; VICTORY_AMBIENT = '0.8'; VICTORY_D3D12_DRED = 'light'; VICTORY_SKIP_PIPELINES = 'none'
}
if ($Safe) {
    $settings.VICTORY_UNDER_WIDE = '0'; $settings.VICTORY_LAND_ECHO = '0'
} else {
    $settings.VICTORY_THIN = '1'; $settings.VICTORY_JOIN_PARTS = '8'; $settings.VICTORY_UPLOAD_AHEAD = '1'
}
if ($RayTracing) { $settings.VICTORY_BVH_BUILDER = '1' }
if ($Console -eq 'series-s') { $settings.VICTORY_DISPLAY_MODE = '1920x1080' }
if ($RenderScale -ne 0 -and $RenderScale -lt 0.2) { throw 'RenderScale must be 0 or between 0.2 and 1.0.' }
$settings.VICTORY_RENDER_SCALE = $RenderScale.ToString('G', [System.Globalization.CultureInfo]::InvariantCulture)
if ($AFMF) { $settings.VICTORY_AFMF = '1' }
foreach ($k in $More.Keys) { $settings[$k] = $More[$k] }
# Stock with explicit scale opts into the separate Foxhound sizing port.
# Scale zero keeps the shipping Series S path untouched.
if ($Upscaler -eq 'amd') {
    if ($Console -ne 'series-s') { throw 'Native AMD upscaling test requires Series S.' }
    if ($settings.VICTORY_DISPLAY_MODE -ne '1920x1080') { throw 'Native AMD upscaling test requires 1080p output.' }
    $ratio = [double]::Parse([string]$settings.VICTORY_RENDER_SCALE, [System.Globalization.CultureInfo]::InvariantCulture)
    if ($ratio -ne 0 -and (-not ($ratio -ge 0.2 -and $ratio -le 0.5))) {
        throw 'Native AMD upscaling accepts stock sizing (0) or custom 0.20 through 0.50 (experimental below 0.50).'
    }
    $settings.VICTORY_RENDER_SCALE = $ratio.ToString('G', [System.Globalization.CultureInfo]::InvariantCulture)
    if (-not $settings.VICTORY_FSR_SDK_DLL -or -not (Test-Path $settings.VICTORY_FSR_SDK_DLL)) {
        throw 'Supply VICTORY_FSR_SDK_DLL in -More.'
    }
    $settings.VICTORY_OWN_UPSCALER = '0'
    $settings.VICTORY_FSR_UPSCALER = '1'
    $settings.VICTORY_FSR_STOCK_SIZING = if ($settings.VICTORY_RENDER_SCALE -eq '0') { '0' } else { '1' }
    $settings.VICTORY_AFMF = '0'
} elseif ($Upscaler -eq 'stock') {
    $settings.VICTORY_OWN_UPSCALER = '0'
    if ($settings.VICTORY_RENDER_SCALE -ne '0') {
        if ($Console -ne 'series-s') { throw 'Experimental stock sizing is only intended for Series S.' }
        $ratio = [double]::Parse([string]$settings.VICTORY_RENDER_SCALE, [System.Globalization.CultureInfo]::InvariantCulture)
        if (-not (@(1.0, (2.0/3.0), 0.58, 0.5, (1.0/3.0)) | Where-Object { [Math]::Abs($_ - $ratio) -lt 0.001 })) {
            throw 'Stock sizing accepts 1, 0.666667, 0.58, 0.5, 0.333333. Other values require -Upscaler game.'
        }
        $settings.VICTORY_FSR_STOCK_SIZING = '1'
    } else {
        $settings.VICTORY_FSR_STOCK_SIZING = '0'
    }
} elseif ($Upscaler -eq 'game') {
    $settings.VICTORY_OWN_UPSCALER = '1'
} elseif (-not $settings.ContainsKey('VICTORY_OWN_UPSCALER')) {
    $settings.VICTORY_OWN_UPSCALER = if ($settings.VICTORY_RENDER_SCALE -ne '0') { '1' } else { '0' }
}

$victory = Join-Path $PSScriptRoot "..\target\$Build\victory.exe"
if (-not (Test-Path $victory)) { Write-Host "No $victory. Build it first (README.md)."; return }
if (-not (Test-Path $Exe)) { Write-Host "No $Exe."; return }
# (Other settings left over in this window go; where your files are stays.)
$paths = 'VICTORY_CONFIG', 'VICTORY_GAME_DIR', 'VICTORY_CACHE_DIR', 'VICTORY_DATA_DIR'
Get-ChildItem Env:VICTORY_* | Where-Object { $_.Name -notin $paths } | ForEach-Object { Remove-Item "Env:$($_.Name)" }
foreach ($k in $settings.Keys) { Set-Item "Env:$k" $settings[$k] }
if ($settings.VICTORY_AFMF -eq '1') {
    Write-Host 'AFMF: enable AMD Fluid Motion Frames in Adrenalin for this host executable:'
    Write-Host (Resolve-Path $victory).Path
    Write-Host 'Keep VSync off; use Alt+R to confirm AFMF active and Ctrl+Shift+O for AMD metrics.'
}
$Exe = (Resolve-Path $Exe).Path
Push-Location (Join-Path $PSScriptRoot '..')   # victory.toml is looked for here
Write-Host "Profile: $Console; display: $($settings.VICTORY_DISPLAY_MODE); render scale: $($settings.VICTORY_RENDER_SCALE); upscaler: $Upscaler; own upscaler: $($settings.VICTORY_OWN_UPSCALER)"
try { & $victory run $Exe --execute --console $Console } finally { Pop-Location }
