# ESP32 firmware

PlatformIO project for the Adafruit ESP32 ESP32 Feather, using the Arduino
framework and the `featheresp32` board target.

Open this folder (`esp32Firmware`) in VS Code with the PlatformIO IDE extension.
Use PlatformIO's Build, Upload, and Serial Monitor actions.

This project can be built on a different computer: install PlatformIO there and
open this folder. PlatformIO downloads the required toolchain on that computer;
no generated `compile_commands.json` or paths from this development machine are
needed. Review the configuration sections at the top of `src/main.cpp`
before building. Constants are build-time settings: changing them requires a
rebuild/upload. DHCP and optional ground sender address learning happen at
runtime; there is currently no runtime settings editor.

Alternatively, run these commands from this folder in a PlatformIO terminal:

```sh
pio run
pio run --target upload
pio device monitor --baud 115200
```

Connect the board with a USB data cable. PlatformIO automatically detects the
upload port; if needed, set `upload_port = COM5` and `monitor_port = COM5` in
`platformio.ini`, replacing `COM5` with your board's port.

The bridge forwards UART bytes to UDP and accepted UDP payloads to UART. It uses
RX GPIO16, TX GPIO17 and the onboard status LED on GPIO13. Cross TX/RX and connect
a common ground to the Teensy. UART settings, credentials, addresses, ports,
buffer sizes and retry/diagnostic intervals are configurable defaults at the top
of `src/main.cpp`; they still require integration review.

Startup does not wait for Wi-Fi. The application loop owns all buffers and
network services; the Wi-Fi callback only publishes a synchronized event counter.
Reconnect attempts are spaced out, UDP is rebound after link/IP changes, and
failed UDP binding and mDNS startup are retried. A local send error closes the
socket and schedules a rebind. Telemetry is drained/discarded while the bridge
is unavailable. Pending application buffers are discarded on service reset.
Bytes already handed to the UART driver cannot be recalled.

Incoming UDP datagrams above `MAX_UDP_PAYLOAD` are discarded whole, without
forwarding a prefix or leaving an unread tail that blocks the next datagram.
One accepted uplink datagram is staged at a time and written only as UART TX
space permits. There is no unbounded application queue; sustained overload can
still lose data in the UART/network drivers.

The LED is solid when Wi-Fi has an IP and blinks while disconnected. It does
not indicate ground/Teensy health. Cumulative serial diagnostics report link
resets, retries, initialization errors, local UDP sends/errors, receives,
oversized/short reads, UART bytes handled, intentional discarded bytes, missing
targets and delimiter batches split by the payload cap. These counters do not
measure UDP delivery, hardware UART overruns or end-to-end loss. Diagnostic
snapshots are written incrementally when console TX space is available.

Packaging modes remain configurable:

- `MODE_IDLE_TIMEOUT` is provisional byte batching based on time since software
  reads. It does not detect wire silence or guarantee one UDP datagram per frame.
- `MODE_DELIMITER` includes the delimiter; batches hitting the size cap may split
  a frame and increment `splits`.
- `MODE_FIXED_SIZE` requires the configured size and endpoint synchronization;
  an interruption or lost UART byte requires an agreed resynchronization policy.

Only these three modes are implemented. Custom framing requires implementing an
agreed parser and adding its mode; there is currently no `MODE_CUSTOM_BLANK`.

`BRIDGE_PACKAGING_MODE` can override the default through a build define. The
project explicitly uses C++17 for its compile-time mode selection.

Integration TODOs: agree telemetry framing/resynchronization, command formats and
validation, sender authorization, sequence/duplicate/expiry rules, discovery and
target ownership, and endpoint link-loss/safety behavior. None are defined by
this transport bridge. In dynamic-target mode, any fully read, size-accepted
datagram updates the telemetry destination to its sender's IP; this is not an
authenticated lock or discovery protocol. Its payload is forwarded unchanged,
and the target is forgotten when services reset. Static-target mode also accepts
incoming datagrams from any sender. mDNS only advertises the ESP32 hostname.

Hardware regression checks (not verified by a successful build):

1. Boot with the router off and continuous UART input; confirm bounded servicing
   and increasing discarded-byte counts, then turn the router on.
2. With a UART capture device instead of an operational endpoint, send a 1200-byte
   UDP datagram, followed by a known <=1024-byte payload. Confirm no prefix of the
   oversized packet is forwarded and the next payload arrives unchanged. Also
   test exactly 1024 bytes and repeated oversized datagrams.
3. Power-cycle the router, change the ESP32 lease, and interrupt the link during
   bidirectional traffic. Confirm rebinding and recovery without replaying staged
   application buffers; test UDP bind failure/retry as well.
4. Exercise continuous UART traffic and saturated uplink traffic together; check
   fairness, diagnostics and endpoint handling of loss and partial frames.
5. Check each implemented packaging mode, independent board resets, dynamic
   target relearning, and USB-console backpressure. Measure actual latency.

Board reference: https://docs.platformio.org/en/latest/boards/espressif32/featheresp32.html
