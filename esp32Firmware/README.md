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
| UART packaging | Idle timeout (provisional) |

The Wi-Fi SSID and password are still placeholders. Set the actual credentials before testing.

The ESP32 uses a static IP because the current `Monoprop/GroundControl` software sends commands to `192.168.1.50`. Verify that the router supports this network configuration and that the address is available.

The laptop's actual IP address also needs to be confirmed before telemetry can be received correctly.

## UART Data Packaging

The Teensy uses Postcard binary serialization for telemetry. The ESP32 forwards the bytes without decoding them.

Three UART-to-UDP packaging modes are currently available:

- `MODE_IDLE_TIMEOUT` — Sends buffered data after a period without UART reads. Currently selected.
- `MODE_DELIMITER` — Sends when a configured delimiter is received.
- `MODE_FIXED_SIZE` — Sends after collecting a configured number of bytes.

**UART framing has not yet been finalized.** The current idle-timeout method does not guarantee that each UDP datagram contains exactly one telemetry message. This must be resolved with the flight-software team before end-to-end integration.

UDP-to-UART forwarding passes accepted UDP payload bytes directly to the Teensy.

## Connection Handling and Diagnostics

The firmware attempts to reconnect automatically when Wi-Fi is unavailable and restarts its UDP service as needed.

Pending application data is discarded during connection resets rather than retransmitted later. UDP packets exceeding the configured maximum payload are rejected.

The status LED indicates Wi-Fi connectivity:

- Solid: Wi-Fi connected with an IP address.
- Blinking: Wi-Fi unavailable.

The Serial Monitor displays periodic connection status, byte counts, packet counts, and error statistics. These are local diagnostics and do not confirm that data reached the destination.

## Integration Items

The following still need to be confirmed:

- UART framing and synchronization with the Teensy.
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

A successful PlatformIO build confirms compilation, not hardware or end-to-end communication.

## Reference

[PlatformIO — Adafruit ESP32 Feather](https://docs.platformio.org/en/latest/boards/espressif32/featheresp32.html)
