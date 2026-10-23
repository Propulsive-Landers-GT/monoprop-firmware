[CmdletBinding()]
param(
    [string]$EspAddress = "192.168.1.50",
    [ValidateRange(1, 65535)]
    [int]$EspPort = 8888,
    [ValidateRange(1, 65535)]
    [int]$LocalPort = 10000,
    [switch]$EchoTest
)

$ErrorActionPreference = "Stop"
$Host.UI.RawUI.WindowTitle = "Monoprop sender - UDP $LocalPort to ${EspAddress}:$EspPort"
$sender = Join-Path $PSScriptRoot "..\.pio\host-tools\monoprop-command-console.exe"
if (!(Test-Path $sender)) { throw "Run tools\build-monitor.ps1 -Tool sender first." }
$senderArgs = @("--bind", "0.0.0.0:$LocalPort", "--target", "${EspAddress}:$EspPort")
if ($EchoTest) { $senderArgs += "--echo" }
& $sender @senderArgs
if ($LASTEXITCODE -ne 0) { throw "Command console failed with code $LASTEXITCODE" }
