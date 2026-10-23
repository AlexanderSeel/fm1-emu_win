param(
    [switch]$NoInstall
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$plugin = Join-Path $root 'crates/fm1-plugin'
$out = Join-Path $root 'target/plugin/windows-x64'
New-Item -ItemType Directory -Force -Path $out | Out-Null

Push-Location $plugin
try {
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Rust CLAP build failed' }
} finally { Pop-Location }

$clapBinary = Join-Path $plugin 'target/release/fm1_plugin.dll'
if (-not (Test-Path $clapBinary)) { throw "Missing CLAP library: $clapBinary" }
$clapFile = Join-Path $out 'FM-1.clap'
Copy-Item $clapBinary $clapFile -Force

$build = Join-Path $out 'build'
cmake -S (Join-Path $plugin 'wrap') -B $build -A x64 -DFM1_CLAP="$clapFile" -DCLAP_WRAPPER_COPY_AFTER_BUILD=OFF
if ($LASTEXITCODE -ne 0) { throw 'CMake configuration failed' }
cmake --build $build --config Release --target fm1_vst3 --parallel 2
if ($LASTEXITCODE -ne 0) { throw 'VST3 wrapper compilation failed' }

$vst3 = Get-ChildItem $build -Recurse -File -Filter 'FM-1.vst3' | Where-Object { $_.FullName -match '[\\/]Release[\\/]' } | Select-Object -First 1
if (-not $vst3) { throw 'Could not find the compiled FM-1.vst3' }
Copy-Item $vst3.FullName (Join-Path $out 'FM-1.vst3') -Force

if (-not $NoInstall) {
    $vstDir = Join-Path $env:COMMONPROGRAMFILES 'VST3'
    New-Item -ItemType Directory -Force -Path $vstDir | Out-Null
    Copy-Item (Join-Path $out 'FM-1.vst3') $vstDir -Force
    # The dynamic CLAP is required by clap-wrapper at runtime.
    Copy-Item $clapFile $vstDir -Force
    Write-Host "Installed FM-1.vst3 and FM-1.clap to $vstDir"
}
Write-Host "Windows binaries: $out"
