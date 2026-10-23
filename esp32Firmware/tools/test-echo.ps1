[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ipaddress]$EspAddress,
    [Parameter(Mandatory = $true)][switch]$EchoFirmwareConfirmed
)
$ErrorActionPreference = "Stop"
if (!$EchoFirmwareConfirmed) { throw "First verify serial shows echo=1 and uart=0." }
if (Get-NetUDPEndpoint -LocalPort 9999 -ErrorAction SilentlyContinue) {
    throw "Close the UDP receiver before running the automated echo test."
}
$tools = Join-Path $PSScriptRoot "..\.pio\host-tools"
$log = Join-Path $env:TEMP ("monoprop-echo-" + [guid]::NewGuid().ToString())
$message = "Monoprop Wi-Fi echo " + [guid]::NewGuid().ToString()
$receiver = $null
$sender = $null
try {
    $receiver = Start-Process (Join-Path $tools "monoprop-telemetry-monitor.exe") `
        -ArgumentList @("--schema", "echo", "--max-packets", "1") `
        -RedirectStandardOutput "$log.json" -RedirectStandardError "$log.err" -PassThru -WindowStyle Hidden
    $null = $receiver.Handle
    $deadline = (Get-Date).AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 100
        $bound = Get-NetUDPEndpoint -LocalPort 9999 -ErrorAction SilentlyContinue |
            Where-Object OwningProcess -eq $receiver.Id
    } while (!$bound -and !$receiver.HasExited -and (Get-Date) -lt $deadline)
    if (!$bound) { throw "Test receiver failed to bind" }
    $info = [System.Diagnostics.ProcessStartInfo]::new()
    $info.FileName = Join-Path $tools "monoprop-command-console.exe"
    $info.Arguments = "--echo --bind 0.0.0.0:0 --target $($EspAddress.IPAddressToString):8888"
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $sender = [System.Diagnostics.Process]::Start($info)
    $sender.StandardInput.WriteLine("send $message")
    $sender.StandardInput.WriteLine("quit")
    $sender.StandardInput.Close()
    if (!$sender.WaitForExit(10000)) { throw "Sender timed out" }
    if ($sender.ExitCode -ne 0) { throw $sender.StandardError.ReadToEnd() }
    if (!$receiver.WaitForExit(15000)) {
        throw "No echo received in 15 seconds. Check Wi-Fi, firewall and client isolation."
    }
    if ($receiver.ExitCode -ne 0) { throw (Get-Content "$log.err" -Raw) }
    $json = Get-Content "$log.json" -Raw
    $result = $json | ConvertFrom-Json
    if ($result.source -ne "$($EspAddress.IPAddressToString):8888" -or
        $result.schema -ne "echo" -or $result.telemetry.sequence -ne 0 -or
        $result.telemetry.message -ne $message) { throw "Echo did not match source, schema, sequence and message" }
    Write-Host "PASS: sender -> ESP32 -> receiver; exact message and sequence matched."
    Write-Host $json
} finally {
    foreach ($p in @($sender, $receiver)) {
        if ($null -ne $p) {
            if (!$p.HasExited) { $p.Kill(); $p.WaitForExit() }
            $p.Dispose()
        }
    }
}
