# Starts the title with the settings this fork is played with (README.md, "How to run it").
#   scripts\play.ps1 -Exe <path of your own copy's executable>
# -Console      series-s (default) or series-x; Series S reports 1080p by default
# -RenderScale  internal scale 0.2..1.0 (0 leaves title default); forces title's own upscaler when enabled
# -Safe         without the newer, faster ways (the first thing to try when something is wrong)
# -RayTracing   the title's own ray-traced indirect light: lit interiors, about a third slower
# -Build        target\diag (the default, the build all measurements are of) or target\release
# -More         other settings: -More @{VICTORY_PANEL='1'}
# -LogDirectory optional log directory (default: repository logs folder)
# Run it from a normal window, not as administrator: the driver then keeps its shader cache elsewhere.
param(
    [Parameter(Mandatory)][string]$Exe,
    [ValidateSet('series-s', 'series-x')][string]$Console = 'series-s',
    [ValidateRange(0.0, 1.0)][double]$RenderScale = 0,
    [switch]$Safe,
    [switch]$RayTracing,
    [ValidateSet('diag', 'release')][string]$Build = 'diag',
    [hashtable]$More = @{},
    [string]$LogDirectory = (Join-Path $PSScriptRoot '..\logs')
)
$settings = @{
    VICTORY_TEXTURE_NO_WAIT = '1'; VICTORY_RELAY = '2'; VICTORY_WIDE_BACK = '1'; VICTORY_CARD_REHASH_RUNS = '1'
    VICTORY_JOIN_COPIES = '1'; VICTORY_INDIRECT_WINDOW = '1'; VICTORY_RELAY_COPY = '1'; VICTORY_CARD_STEPS = '2'
    VICTORY_INDIRECT_BATCH = '2'; VICTORY_MAP_THREADS = '1'; VICTORY_TITLE_PRIORITIES = '0'; VICTORY_CARD_AWAKE = '1'
    VICTORY_SHOW_AFTER = '1'; VICTORY_AMBIENT = '0.8'; VICTORY_D3D12_DRED = 'light'; VICTORY_SKIP_PIPELINES = 'none'
}
# Start with conservative renderer settings to isolate Series S presentation.
# All cumulative experimental ports remain available through -More.
$settings.VICTORY_PLANES = '1'
$settings.VICTORY_PORT_CONSTANTS = '1'
$settings.VICTORY_PORT_DESCRIPTORS = '0'
$settings.VICTORY_PORT_CBV = '0'
$settings.VICTORY_PORT_CBV_PLACE = '0'
$settings.VICTORY_PORT_CBV_RELAY2 = '0'
$settings.VICTORY_PORT_GUARD = '0'
$settings.VICTORY_UPLOAD_WORKERS = '2'
if ($Console -eq 'series-s') { $settings.VICTORY_DISPLAY_MODE = '1920x1080' }
if ($Safe) {
    $settings.VICTORY_UNDER_WIDE = '0'; $settings.VICTORY_LAND_ECHO = '0'
} else {
    $settings.VICTORY_THIN = '1'; $settings.VICTORY_JOIN_PARTS = '8'; $settings.VICTORY_UPLOAD_AHEAD = '1'
}
if ($RayTracing) { $settings.VICTORY_BVH_BUILDER = '1' }
if ($RenderScale -ne 0 -and $RenderScale -lt 0.2) { throw 'RenderScale must be 0 or between 0.2 and 1.0.' }
$settings.VICTORY_RENDER_SCALE = $RenderScale.ToString('G', [System.Globalization.CultureInfo]::InvariantCulture)
foreach ($k in $More.Keys) { $settings[$k] = $More[$k] }
if (-not $settings.ContainsKey('VICTORY_OWN_UPSCALER')) {
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
try {
    $null = New-Item -ItemType Directory -Path $LogDirectory -Force -ErrorAction Stop
    $LogFile = Join-Path (Resolve-Path $LogDirectory).Path ("victory-{0}-{1}.log" -f (Get-Date -Format 'yyyyMMdd-HHmmss-fff'), $PID)
    Write-Host "Logging to $LogFile"
    "Profile: $Console; display: $($settings.VICTORY_DISPLAY_MODE); planes: $($settings.VICTORY_PLANES); render scale: $($settings.VICTORY_RENDER_SCALE); own upscaler: $($settings.VICTORY_OWN_UPSCALER)" | Tee-Object -FilePath $LogFile
    & $victory run $Exe --execute --console $Console 2>&1 | Tee-Object -FilePath $LogFile -Append
    $code = $LASTEXITCODE
} finally { Pop-Location }
exit $code
