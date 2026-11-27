# Starts the title with the settings this fork is played with (README.md, "How to run it").
#   scripts\play.ps1 -Exe <path of your own copy's executable>
# -Console      series-x (default) or series-s (1080p reported by default)
# -RenderScale  0 (stock) or 0.2..1.0 (internal scale)
# -Upscaler     auto (default), game (title temporal), stock (console default)
# -Safe         without the newer, faster ways (the first thing to try when something is wrong)
# -RayTracing   the title's own ray-traced indirect light: lit interiors, about a third slower
# -Build        target\diag (the default, the build all measurements are of) or target\release
# -More         other settings: -More @{VICTORY_PANEL='1'}
# Run it from a normal window, not as administrator: the driver then keeps its shader cache elsewhere.
param(
    [Parameter(Mandatory)][string]$Exe,
    [ValidateSet('series-x', 'series-s')][string]$Console = 'series-x',
    [ValidateRange(0.0, 1.0)][double]$RenderScale = 0,
    [ValidateSet('auto', 'game', 'stock')][string]$Upscaler = 'auto',
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
foreach ($k in $More.Keys) { $settings[$k] = $More[$k] }
# Keep the stock Series S FSR path unmodified; its custom-scale hook is experimental.
if ($Upscaler -eq 'stock') {
    if ($settings.VICTORY_RENDER_SCALE -ne '0') { throw 'Stock upscaler test requires RenderScale 0. Use -Upscaler game for custom internal scale.' }
    $settings.VICTORY_OWN_UPSCALER = '0'
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
$Exe = (Resolve-Path $Exe).Path
Push-Location (Join-Path $PSScriptRoot '..')   # victory.toml is looked for here
Write-Host "Profile: $Console; display: $($settings.VICTORY_DISPLAY_MODE); render scale: $($settings.VICTORY_RENDER_SCALE); upscaler: $Upscaler; own upscaler: $($settings.VICTORY_OWN_UPSCALER)"
try { & $victory run $Exe --execute --console $Console } finally { Pop-Location }
