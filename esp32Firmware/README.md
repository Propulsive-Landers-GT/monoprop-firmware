# HUZZAH32 firmware

PlatformIO project for the Adafruit HUZZAH32 ESP32 Feather, using the Arduino
framework and the `featheresp32` board target.

Open this folder (`esp32Firmware`) in VS Code with the PlatformIO IDE extension.
Use PlatformIO's Build, Upload, and Serial Monitor actions.

Alternatively, run these commands from this folder in a PlatformIO terminal:

```sh
pio run
pio run --target upload
pio device monitor --baud 115200
```

Connect the board with a USB data cable. PlatformIO automatically detects the
upload port; if needed, set `upload_port = COM5` and `monitor_port = COM5` in
`platformio.ini`, replacing `COM5` with your board's port.

The starter sketch prints a startup message at 115200 baud and leaves GPIO pins
unconfigured. Add application code in `src/main.cpp`, headers in `include/`,
and project libraries in `lib/`.

Board reference: https://docs.platformio.org/en/latest/boards/espressif32/featheresp32.html
