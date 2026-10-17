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

The Wi-Fi SSID and password are still placeholders. Set the actual credentials before testing.

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
the ground station must include the COBS terminator. The inspected Monoprop
endpoints still use raw Postcard serialization and must be updated separately.
This ESP32 change alone does not make them compatible. Endpoints must also
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

- Implement the same Postcard COBS convention at both endpoints, including delimiter handling and resynchronization.
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
