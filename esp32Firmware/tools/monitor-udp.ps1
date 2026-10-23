[CmdletBinding()]
param(
    [ValidateRange(1, 65535)]
    [int]$Port = 9999,
    [string]$BindAddress = "0.0.0.0",
    [ValidateRange(0, 2147483647)]
    [int]$MaxPackets = 0,
    [ValidateSet("modern", "legacy", "echo")]
    [string]$Schema = "modern",
    [switch]$Compact,
    [switch]$Raw
)

$ErrorActionPreference = "Stop"
$Host.UI.RawUI.WindowTitle = "Monoprop ESP32 telemetry - UDP $Port"
if (!$Raw) {
    $decoder = Join-Path $PSScriptRoot "..\.pio\host-tools\monoprop-telemetry-monitor.exe"
    if (!(Test-Path $decoder)) {
        throw "Postcard decoder is not built. Run tools\build-monitor.ps1 first, or use -Raw for byte diagnostics."
    }
    $decoderArgs = @("--bind", "${BindAddress}:$Port", "--schema", $Schema, "--max-packets", "$MaxPackets")
    if (!$Compact) { $decoderArgs += "--pretty" }
    & $decoder @decoderArgs
    if ($LASTEXITCODE -ne 0) { throw "Postcard monitor exited with code $LASTEXITCODE" }
    return
}

$receiver = [System.Net.Sockets.UdpClient]::new()
try {
    # Exclusive binding prevents two monitors from silently splitting packets.
    $receiver.ExclusiveAddressUse = $true
    $receiver.Client.ReceiveTimeout = 1000
    $endpoint = [System.Net.IPEndPoint]::new([System.Net.IPAddress]::Parse($BindAddress), $Port)
    $receiver.Client.Bind($endpoint)
    Write-Host "Listening on ${BindAddress}:$Port (UDP). Ctrl+C to stop."
    Write-Host "Receive-only: no commands or discovery packets are sent."
    Write-Host "Waiting is normal until Wi-Fi is configured and Teensy telemetry arrives."
    Write-Host "Frames are raw bytes, not decoded Postcard messages."
    $count = 0
    while ($MaxPackets -eq 0 -or $count -lt $MaxPackets) {
        $remote = [System.Net.IPEndPoint]::new([System.Net.IPAddress]::Any, 0)
        try {
            $bytes = $receiver.Receive([ref]$remote)
        } catch [System.Net.Sockets.SocketException] {
            if ($_.Exception.SocketErrorCode -eq [System.Net.Sockets.SocketError]::TimedOut) {
                continue
            }
            throw
        }
        $count++
        $hex = [System.BitConverter]::ToString($bytes).Replace("-", " ")
        # Do not render terminal control sequences from untrusted payloads.
        $ascii = -join ($bytes | ForEach-Object {
            if ($_ -ge 32 -and $_ -le 126) { [char]$_ } else { "." }
        })
        Write-Host ("[{0:HH:mm:ss.fff}] {1} {2} bytes" -f (Get-Date), $remote, $bytes.Length)
        Write-Host "  HEX   $hex"
        Write-Host "  ASCII $ascii"
    }
} finally {
    $receiver.Dispose()
}
