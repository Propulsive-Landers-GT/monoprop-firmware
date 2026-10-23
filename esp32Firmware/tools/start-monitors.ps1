[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^COM[0-9]+$')]
    [string]$SerialPort,
    [ValidateRange(1, 65535)]
    [int]$UdpPort = 9999,
    [ValidateSet("modern", "legacy", "echo")]
    [string]$Schema = "modern",
    [switch]$Raw,
    [switch]$Compact
)

$ErrorActionPreference = "Stop"
if ($SerialPort -notin [System.IO.Ports.SerialPort]::GetPortNames()) {
    throw "$SerialPort is not present. Check Device Manager for the ESP32's COM port."
}
if (Get-NetUDPEndpoint -LocalPort $UdpPort -ErrorAction SilentlyContinue) {
    throw "UDP $UdpPort is already in use. Close the existing receiver first."
}
$putty = Get-Command putty.exe -ErrorAction SilentlyContinue
if (!$putty) {
    throw "PuTTY is not on PATH. Install PuTTY or launch it manually at 115200 baud, 8N1, no flow control."
}
$receiver = Join-Path $PSScriptRoot "monitor-udp.ps1"
if (!$Raw -and !(Test-Path (Join-Path $PSScriptRoot "..\.pio\host-tools\monoprop-telemetry-monitor.exe"))) {
    throw "Run tools\build-monitor.ps1 first to build the Postcard decoder, or use -Raw."
}
$powershell = Join-Path $PSHOME "powershell.exe"
$receiverArgs = @(
    "-NoProfile", "-NoExit", "-ExecutionPolicy", "Bypass",
    "-File", "`"$receiver`"", "-Port", "$UdpPort", "-Schema", $Schema
)
if ($Raw) { $receiverArgs += "-Raw" }
if ($Compact) { $receiverArgs += "-Compact" }
Start-Process -FilePath $powershell -ArgumentList $receiverArgs
Start-Process -FilePath $putty.Source -ArgumentList @(
    "-serial", $SerialPort, "-sercfg", "115200,8,n,1,N"
)
Write-Host "Opened USB diagnostics on $SerialPort and a receive-only UDP $UdpPort monitor."
Write-Host "Close PuTTY before uploading firmware again."
