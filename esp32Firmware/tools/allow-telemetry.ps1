#Requires -RunAsAdministrator
[CmdletBinding()]
param(
    [ValidateRange(1, 65535)]
    [int]$Port = 9999
)

$ErrorActionPreference = "Stop"
$name = "Monoprop-ESP32-Telemetry-UDP-$Port"
$existing = Get-NetFirewallRule -Name $name -ErrorAction SilentlyContinue
if ($existing) {
    throw "Firewall rule '$name' already exists. Inspect it instead of silently replacing it."
}
New-NetFirewallRule -Name $name -DisplayName "Monoprop ESP32 telemetry (UDP $Port, private LAN)" `
    -Direction Inbound -Action Allow -Protocol UDP -LocalPort $Port `
    -Profile Private -RemoteAddress LocalSubnet -EdgeTraversalPolicy Block | Out-Null
Write-Host "Allowed inbound UDP $Port from the local subnet on Private networks only."
Write-Host "No rule was added for Public networks. No router port forwarding is needed."
