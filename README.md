# monoprop-firmware

**Embedded flight firmware** for the Monoprop UAV / lander avionics — the code
that runs on the microcontrollers.

## Contents

| Path | What it is |
|------|------------|
| `firmware/` | Primary [PlatformIO](https://platformio.org/) project: `src/` (incl. `CustomLinear/`), `include/`, `lib/`, `test/`, `platformio.ini`, helper shell scripts |
| [`esp32Firmware/`](esp32Firmware/README.md) | UART-to-UDP bridge between the Teensy and ground station |
| `AvionicsFirmware/` | VCSEL sensor Arduino sketch draft |
| `include/`, `lib/` | Top-level PlatformIO include/lib placeholders |
| `build/` | Committed CMake build tree (kept intact, not regenerated) |
| `.gitmodules` | Submodule references: `Adafruit_VL53L0X`, `MPU9250` (under `firmware/lib/`) |

> **Note:** `build/` contains committed CMake artifacts; consider `.gitignore`-ing
> and removing them in a future cleanup.

## Submodules

```bash
git submodule update --init --recursive
```

## Related repos

- [`monoprop-electronics`](../monoprop-electronics) — avionics hardware / PCBs this firmware runs on
- [`monoprop-flight-software`](../monoprop-flight-software) — real-time integrated control loop
- [`navigation`](../navigation) — state estimation / attitude estimation (vehicle-agnostic library)
