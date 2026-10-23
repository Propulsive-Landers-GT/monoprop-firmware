#Requires -RunAsAdministrator
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ipaddress]$EspAddress,
    [Parameter(Mandatory = $true)][ipaddress]$LocalAddress
)
$ErrorActionPreference = "Stop"
$name = "Monoprop-ESP32-Echo-UDP-9999"
if (Get-NetFirewallRule -Name $name -ErrorAction SilentlyContinue) {
    throw "Rule $name already exists. Inspect/remove that test rule before changing its addresses."
}
New-NetFirewallRule -Name $name -DisplayName "Monoprop ESP32 echo test (specific peer)" `
    -Direction Inbound -Action Allow -Protocol UDP -LocalPort 9999 -RemotePort 8888 `
    -LocalAddress $LocalAddress.IPAddressToString -RemoteAddress $EspAddress.IPAddressToString `
    -InterfaceAlias "Wi-Fi" -Profile Any -EdgeTraversalPolicy Block | Out-Null
Write-Host "Allowed only $EspAddress`:8888 -> $LocalAddress`:9999 over Wi-Fi."
