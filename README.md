# monoprop-firmware

**Embedded flight firmware** for the Monoprop UAV / lander avionics — the code
that runs on the microcontrollers.

> **Provenance:** Split out of the monolithic
> [`GTPL-Testing/MonopropUAV`](https://github.com/GTPL-Testing/MonopropUAV)
> repository during the July 2026 reorganization. Full commit history for these
> files is preserved. See the [GTPL-test root README](../README.md) for the
> complete old→new mapping.

## Contents

| Path | What it is |
|------|------------|
| `firmware/` | Primary [PlatformIO](https://platformio.org/) project: `src/` (incl. `CustomLinear/`), `include/`, `lib/`, `test/`, `platformio.ini`, helper shell scripts |
| `AvionicsFirmware/` | VCSEL sensor Arduino sketch draft |
| `include/`, `lib/` | Top-level PlatformIO include/lib placeholders carried over from the original repo |
| `build/` | CMake build tree carried over from the original repo (kept intact, not regenerated) |
| `.gitmodules` | Submodule references: `Adafruit_VL53L0X`, `MPU9250` (under `firmware/lib/`) |

> **Note:** `build/` contains committed CMake artifacts inherited from the original
> repository. They were preserved rather than deleted per the reorg's no-delete rule;
> consider `.gitignore`-ing and removing them in a future cleanup.

## Submodules

```bash
git submodule update --init --recursive
```

## Related repos

- [`monoprop-electronics`](../monoprop-electronics) — avionics hardware / PCBs this firmware runs on
- [`monoprop-flight-software`](../monoprop-flight-software) — real-time integrated control loop
- [`navigation`](../navigation) — state estimation / attitude estimation (vehicle-agnostic library)
