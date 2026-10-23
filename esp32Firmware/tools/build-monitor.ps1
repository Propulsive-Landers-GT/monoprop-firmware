[CmdletBinding()]
param(
    [ValidateSet("all", "monitor", "sender")]
    [string]$Tool = "all"
)

$ErrorActionPreference = "Stop"
$project = Split-Path $PSScriptRoot -Parent
$manifest = Join-Path $project "telemetry-monitor\Cargo.toml"
# Keep MSVC intermediate paths off the WSL UNC share.
$targetDir = Join-Path $env:LOCALAPPDATA "Monoprop\telemetry-monitor-target"
$names = @()
if ($Tool -ne "sender") { $names += "monoprop-telemetry-monitor" }
if ($Tool -ne "monitor") { $names += "monoprop-command-console" }
$destination = Join-Path $project ".pio\host-tools"
New-Item -ItemType Directory -Force -Path $destination | Out-Null
foreach ($name in $names) {
    cargo build --release --locked --bin $name --manifest-path $manifest --target x86_64-pc-windows-msvc --target-dir $targetDir
    if ($LASTEXITCODE -ne 0) { throw "Build failed: $name" }
    Copy-Item (Join-Path $targetDir "x86_64-pc-windows-msvc\release\$name.exe") $destination -Force
}
Write-Host "Built $($names -join ', ') in $destination. Close each console before rebuilding its executable."
