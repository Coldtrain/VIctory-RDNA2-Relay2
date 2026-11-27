# Run from an x64 Visual Studio Developer PowerShell.
param(
    [Parameter(Mandatory)][string]$SdkRoot,
    [Parameter(Mandatory)][string]$Dll
)
$ErrorActionPreference = 'Stop'
$sdk = (Resolve-Path $SdkRoot).Path
$library = (Resolve-Path $Dll).Path
$include = Join-Path $sdk 'ffx-api\include'
if (-not (Test-Path (Join-Path $include 'ffx_api\ffx_framegeneration.h'))) {
    throw 'SDK root must contain ffx-api/include from FidelityFX SDK v1.1.4.'
}
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    throw 'Run in an x64 Visual Studio Developer PowerShell with MSVC installed.'
}
$source = Join-Path $PSScriptRoot '..\tools\fsr31-probe.cpp'
$out = Join-Path ([System.IO.Path]::GetTempPath()) ('victory-fsr31-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $out
Push-Location $out
try {
    & cl.exe /nologo /std:c++17 /EHsc "/I$include" $source /Fe:fsr31-probe.exe /link d3d12.lib dxgi.lib
    if ($LASTEXITCODE -ne 0) { throw "MSVC failed: $LASTEXITCODE" }
    & .\fsr31-probe.exe $library
    if ($LASTEXITCODE -ne 0) { throw "FidelityFX probe failed: $LASTEXITCODE" }
} finally {
    Pop-Location
    Remove-Item -Recurse -Force $out
}
