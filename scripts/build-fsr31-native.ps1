# Build the native backend against AMD FidelityFX SDK v1.1.4, using x64 MSVC.
param([Parameter(Mandatory)][string]$SdkRoot)
$ErrorActionPreference = 'Stop'
$include = Join-Path (Resolve-Path $SdkRoot).Path 'ffx-api\include'
if (-not (Test-Path (Join-Path $include 'ffx_api\ffx_framegeneration.h'))) { throw 'SDK headers missing.' }
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) { throw 'Use x64 Visual Studio Developer PowerShell.' }
$source = (Resolve-Path (Join-Path $PSScriptRoot '..\tools\fsr31-native.cpp')).Path
$out = Join-Path $PSScriptRoot '..\target\fsr31-native'
$null = New-Item -ItemType Directory -Force -Path $out
Push-Location $out
try {
    & cl.exe /nologo /std:c++17 /EHsc /LD "/I$include" $source /link /OUT:victory_fsr31_native.dll d3d12.lib dxgi.lib
    if ($LASTEXITCODE -ne 0) { throw "MSVC failed: $LASTEXITCODE" }
} finally { Pop-Location }
