# ESP32 UART–UDP Bridge Firmware

Firmware for the Adafruit ESP32 Feather that provides bidirectional communication between the Teensy flight controller and the ground station over Wi-Fi.

The ESP32 acts as a transport bridge and does not interpret telemetry or command data.

```text
Teensy <--UART--> ESP32 <--Wi-Fi/UDP--> Ground Station
```

## Building and Uploading

This project uses PlatformIO with the Arduino framework and the `featheresp32` board target.

Open the `esp32Firmware` directory in VS Code with the PlatformIO extension installed.

Alternatively, run the following commands from the project directory:

```bash
pio run
pio run --target upload
pio device monitor --baud 115200
```

Connect the ESP32 using a USB data cable. If PlatformIO does not detect the correct serial port, specify `upload_port` and `monitor_port` in `platformio.ini`.

All communication settings are defined near the top of `src/main.cpp`. Changes require rebuilding and uploading the firmware.

### Windows Monitors (Including WSL Builds)

Keep USB attached to Windows when using PuTTY. A CP2104 USB-to-UART bridge
requires the [Silicon Labs CP210x VCP driver](https://www.silabs.com/software-and-tools/usb-to-uart-bridge-vcp-drivers).
Use the COM port shown for that device in Device Manager, not a Bluetooth COM
port. An ESP32 uses `.bin` flash images, not the Teensy's `.hex` format.

Build the native Windows Postcard decoder once (Windows Rust/MSVC toolchain
required). Keep `monoprop-prototype-software` checked out beside this repository:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\build-monitor.ps1
```

From Windows PowerShell in this directory, replacing `COM7` with the actual port:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\start-monitors.ps1 -SerialPort COM7
```

This opens PuTTY at 115200 baud, 8N1, no flow control, plus a separate receive-only
Postcard/COBS UDP monitor on port 9999. The scripts can also run from the project's WSL UNC
path. Close PuTTY before flashing again; only one process can own the COM port.

The USB console shows bridge diagnostics. This firmware does **not** forward
typed USB input to the Teensy. Commands are accepted on the ESP32's UDP port
8888 and forwarded directly to UART; do not send arbitrary test packets there.

For incoming telemetry on a trusted field network, run this once from an
**Administrator** PowerShell window:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\allow-telemetry.ps1
```

The firewall rule permits UDP 9999 only from the local subnet on **Private**
Windows network profiles. It does not expose that port on Public networks or
change any network's category. Use a trusted private LAN when network details
are available; do not relax the firewall for a campus/public network. To undo:

```powershell
Remove-NetFirewallRule -Name Monoprop-ESP32-Telemetry-UDP-9999
```

The receiver runs natively on Windows, so no WSL port forwarding is necessary.
It removes COBS framing, deserializes Postcard using the shared Teensy types, and
displays named fields as indented JSON with the sender address. JSON is only the
computer-side display format; the wire stays binary Postcard/COBS. Run it separately:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\monitor-udp.ps1 -Port 9999
```

The default `-Schema modern` matches `TELEMETRY_FORMAT = Modern` in the current
prototype firmware and imports `monoprop_common::TelemetryFrame` directly.
Use `-Schema legacy` only when the Teensy is configured to send
`LegacyDroneTelemetry` (the same field layout as the original
`Monoprop/Common::DroneTelemetry`). The decoder does not guess the schema.
Modern `attitude_quat` is `[qw, qx, qy, qz]`; legacy `orientation` is
`[qx, qy, qz, qw]`. Names, units and ordering remain those of the shared types.
Rebuild the monitor whenever those shared types change.

Add `-Compact` for one JSON record per line, suitable for logging or external
control-loop consumers. Add `-Raw` to restore the hex/ASCII diagnostic view.
These options also work with `start-monitors.ps1`. USB remains plain-text bridge
diagnostics in PuTTY; the ESP32 does not send Postcard telemetry on USB Serial.

The Rust decoder rejects datagrams larger than 1024 bytes, missing delimiters,
multiple frames in a datagram, invalid COBS/Postcard, and trailing serialized
payload. Rejected packets are reported on stderr; the next datagram is still
processed. It never replies or sends control commands. This framing has no
checksum, authentication, or schema/version tag: successful decoding alone
does not establish data integrity, sender identity, or flight safety.

Run host tests from WSL or Windows (without a Teensy target override):

```text
cargo test --locked --manifest-path telemetry-monitor/Cargo.toml
```

The monitor library exposes `decode_datagram` for a future external control
loop, separate from the UDP socket and JSON presentation. Its shared-type
dependency also builds flight-core and the pinned estimator dependency; it
does not execute sensor fusion or a controller on the computer.

An open UDP port alone does not establish Wi-Fi. Until real Wi-Fi credentials,
ESP32 network settings, and the Windows Wi-Fi adapter's telemetry destination
address are configured in `src/main.cpp` and flashed, the receiver will wait.
Even after connection, this bridge sends telemetry only when the Teensy supplies
UART frames; it does not transmit Wi-Fi heartbeats. A local receiver test is not
an end-to-end radio or flight-controller test.

## Receive And Send

### Wi-Fi Echo Bench Test

For a PC -> ESP32 -> PC round trip without the Teensy, build and flash the
**separate** `featheresp32-echo` PlatformIO environment:

```text
pio run -e featheresp32-echo
```

Flash images from `.pio/build/featheresp32-echo`, not the normal bridge directory.
This build uses your configured Wi-Fi credentials with DHCP. Serial prints its
assigned IP and `echo=1 link=1 udp=1 uart=0`. UART is not initialized and no
packets are forwarded to the Teensy. Each accepted UDP packet on 8888 is echoed
unchanged to **the sender's IP on port 9999**, not the sender's source port.
It does not use the normal fixed telemetry destination for echo replies.

After verifying that serial banner, open a receiver with `-Schema echo` and a
sender with `-EchoTest -EspAddress <assigned-IP>`:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\monitor-udp.ps1 -Schema echo
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\send-commands.ps1 -EchoTest -EspAddress 10.91.156.45
```

The IP above is an example from this bench session; read it again after changing
networks. In the echo sender type `send hello ESP32`. The host serializes an
`EchoMessage { sequence, message }` with Postcard/COBS. The receiver decodes the
returned bytes and prints the message, sequence and source. An echo confirms
the Wi-Fi round trip, **not** Teensy receipt or execution.

For an automated round-trip check, close the receiver, then run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\test-echo.ps1 -EspAddress 10.91.156.45 -EchoFirmwareConfirmed
```

This sends a unique test message and requires an exact match of source IP/port,
sequence and message within 15 seconds. Never run it against normal flight
bridge firmware. The echo payload is not a flight command protocol.

If the test Wi-Fi is classified Public, the earlier Private-network firewall
rule is inactive. An Administrator can run `allow-echo.ps1 -EspAddress <ESP-IP>
-LocalAddress <PC-IP>` to allow only that peer's UDP 8888 -> 9999 traffic over
the Wi-Fi interface. It does not change the network profile. Remove the bench
rule afterward with `Remove-NetFirewallRule -Name Monoprop-ESP32-Echo-UDP-9999`.

Reflash `featheresp32` to restore the normal Teensy bridge. Its original static
IP configuration remains separate and must be configured for the flight LAN.

### Flight Bridge Consoles

Run these in two Windows PowerShell windows from `esp32Firmware`:

```powershell
# Receive telemetry on the computer's UDP 9999.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\monitor-udp.ps1

# Send from local UDP 10000 to the ESP32's UDP 8888.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\send-commands.ps1 -EspAddress 192.168.1.50
```

`build-monitor.ps1` builds both tools; use `-Tool sender` to build only the sender
without replacing an open monitor executable. USB/PuTTY is a third, independent
diagnostic channel. The sender does not need an inbound firewall rule: telemetry
still arrives on 9999, not on its source port 10000.

The sender opens its socket but transmits nothing until an explicit `send` line:

```text
send {"kind":"text","command":"Disarm"}
```

This is a real flight-state command, **not** a connectivity test. `help` lists
supported commands; `quit` closes the console without sending anything or
disarming the vehicle. Use only intentional commands with hardware made safe.
For structured control, use `{"kind":"control","command":{...}}` with the
shared `monoprop_common::ControlCommand` fields, including an explicit sequence
number. Numeric commands must be finite, normalized thrust must be 0..1, and
gimbal angles must stay within the shared TVC limits. Only the inner shared
command is Postcard/COBS encoded; the JSON `kind` envelope never goes on the wire.

The sender is manual, not a timed control loop or heartbeat generator. Successful
UDP submission does not confirm delivery or execution. Both computers' network
settings and the ESP32 telemetry destination must still match the actual LAN;
opening these ports does not establish the Wi-Fi connection.

## Hardware Connections

Connect the Teensy and ESP32 using UART with a shared ground.

| Teensy | ESP32 |
|---|---|
| TX (pin 1) | RX (GPIO16) |
| RX (pin 0) | TX (GPIO17) |
| GND | GND |

Both devices use 3.3V UART logic. The ESP32's GPIO13 is currently configured as a status LED output; confirm the LED wiring on the selected board.

## Current Communication Settings

These settings match the existing Monoprop implementation where possible. Values that have not been confirmed with the team are marked as provisional.

| Setting | Value |
|---|---|
| UART baud rate | 115200 (8N1) |
| ESP32 IP address | 192.168.1.50 (static) |
| UDP command listening port | 8888 |
| UDP telemetry destination port | 9999 |
| Ground station IP | 192.168.1.100 (provisional) |
| Router gateway | 192.168.1.1 (provisional) |
| Subnet mask | 255.255.255.0 (provisional) |
| Maximum UDP payload | 1024 bytes |
| UART packaging | Postcard COBS frames, terminated by `0x00` |

Set the actual Wi-Fi SSID and password in `src/main.cpp` before testing.

The ESP32 uses a static IP because the current `Monoprop/GroundControl` software sends commands to `192.168.1.50`. Verify that the router supports this network configuration and that the address is available.

The laptop's actual IP address also needs to be confirmed before telemetry can be received correctly.

## UART Data Packaging

Use Postcard's built-in COBS serialization/deserialization at both endpoints.
The ESP32 neither encodes nor decodes COBS or Postcard: it forwards encoded bytes
unchanged, including the final `0x00` delimiter in each telemetry UDP datagram.

Three UART-to-UDP packaging modes are currently available:

- `MODE_IDLE_TIMEOUT` — Sends buffered data after a period without UART reads; does not preserve message boundaries.
- `MODE_DELIMITER` — Currently selected, with `FRAME_DELIMITER = 0x00` for Postcard COBS.
- `MODE_FIXED_SIZE` — Sends after collecting a configured number of bytes.

Encoded frames must fit within 1024 bytes **including the delimiter**. An oversized
UART frame is discarded through its next delimiter, rather than forwarded as
partial UDP packets. The existing `splits` diagnostic counts these discarded
oversized frames; `dropUART` counts their discarded bytes.

UDP-to-UART forwarding passes accepted UDP payload bytes directly to the Teensy;
the ground station must include the COBS terminator. The original
`monoprop-flight-software/Monoprop/Drone` and `GroundControl` endpoints still use
raw Postcard serialization and must be updated separately; the current
`monoprop-prototype-software` Teensy link already uses COBS. This host monitor
does not change the original controller endpoints. Endpoints must also
recover from lost bytes or starting/reconnecting partway through a UART frame;
the bridge does not validate COBS frames or guarantee alignment after a reset.

## Connection Handling and Diagnostics

The firmware attempts to reconnect automatically when Wi-Fi is unavailable and
rebinds UDP after link/IP changes. A failed UDP send increments `txErrors` and
discards only that downlink batch; the socket and network services stay open.

Pending application data is discarded during connection resets rather than retransmitted later. UDP packets exceeding the configured maximum payload are rejected.

The status LED indicates Wi-Fi connectivity:

- Solid: Wi-Fi connected with an IP address.
- Blinking: Wi-Fi unavailable.

The Serial Monitor displays periodic connection status, byte counts, packet counts, and error statistics. These are local diagnostics and do not confirm that data reached the destination.

The main loop has no artificial delay. UART processing remains limited to
`UART_WORK_PER_PASS` bytes per direction per pass, allowing multiple queued
frames to be sent within that budget. Diagnostic output remains incremental.

## Integration Items

The following still need to be confirmed:

- Confirm the deployed Teensy matches the selected monitor schema and uses COBS framing; the original Drone/GroundControl endpoints need migration if used.
- Ground station IP address and router configuration.
- Final ESP32 hardware pin assignments.
- Expected telemetry throughput and acceptable communication latency.

The flight software is responsible for interpreting telemetry and validating commands. The ESP32 firmware does not implement those application-level protocols.

## Testing

Before hardware integration, verify:

1. Teensy UART data is forwarded to the correct ground-station UDP port.
2. UDP data from the ground station is forwarded correctly over UART.
3. Oversized UDP packets are rejected without affecting subsequent packets.
4. Wi-Fi disconnection and reconnection do not cause stale data to be forwarded.
5. Sustained bidirectional communication works without unexpected data loss.
6. Back-to-back COBS frames retain their delimiters, and oversized UART frames are discarded without affecting the next frame.
7. Injected UDP send failures leave services open; verify recovery and watchdog/task scheduling with the delay removed.

A successful PlatformIO build confirms compilation, not hardware or end-to-end communication.

## Reference

[PlatformIO — Adafruit ESP32 Feather](https://docs.platformio.org/en/latest/boards/espressif32/featheresp32.html)
