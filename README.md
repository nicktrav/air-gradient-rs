# air-gradient-rs

Custom `no_std` Rust firmware for an **AirGradient Open Air O-1PST** outdoor air
quality monitor (**ESP32-C3-MINI**, RISC-V `RV32IMC`).

This is a learning project built in small, reviewable **wedges**. The current wedge
is a **serial heartbeat**: the board boots and prints a counted heartbeat line over
the C3's native USB-Serial-JTAG console. No Wi-Fi, MQTT, Embassy, or sensor drivers
yet — just the skeleton plus a full test/CI harness proven end to end.

See [`CLAUDE.md`](./CLAUDE.md) for the authoritative architecture, hardware notes,
and conventions.

## Layout

```
core/        aq-core      — pure, #![no_std], host-testable logic (no esp-hal)
firmware/    aq-firmware  — thin esp-hal adapter that runs on the MCU
flake.nix    Nix dev shell providing the whole toolchain
```

`aq-core` holds anything that can be plain data-in/data-out (formatting today;
sensor decode and correction math later) so it runs under a normal `cargo test`.
`aq-firmware` is boring glue.

## Development environment

The toolchain (Rust pinned via `rust-toolchain.toml`, plus `espflash`, `esptool`,
`cargo-insta`) is provided by a **Nix flake**. Nothing is installed globally.

```sh
nix develop                      # enter the dev shell
# or prefix individual commands:
nix develop --command cargo test
```

> Nix flakes only see **git-tracked** files. After creating a new file, `git add`
> it before Nix will pick it up.

If you prefer not to use the shell interactively, [`direnv`](https://direnv.net/)
with `echo 'use flake' > .envrc && direnv allow` loads it automatically on `cd`.

## Build & flash

> **Back up the stock firmware first — see [Safety](#safety-read-before-flashing).**

```sh
cd firmware
cargo run            # builds for riscv32imc, flashes over USB-C, opens the monitor
```

`firmware/.cargo/config.toml` sets the MCU target and wires `espflash flash
--monitor` as the cargo runner. You should see, once per second:

```
[aq] heartbeat #0 up 0:00:00
[aq] heartbeat #1 up 0:00:01
...
```

The console re-enumerates on every reset (it *is* the USB-Serial-JTAG), so the
monitor briefly drops and reconnects each boot — expected, not a bug.

To build the firmware without flashing (what CI does):

```sh
cargo build -p aq-firmware --target riscv32imc-unknown-none-elf --release
```

## Tests

All host tests run on your machine — no board, no emulator:

```sh
cargo test                       # aq-core unit + snapshot + driver-trait tests
UPDATE_EXPECT=1 cargo test       # rewrite inline expect-test golden values
cargo insta review               # review/accept changed insta snapshots
```

Layers, cheapest first:

1. **`aq-core` host tests** — `expect-test` inline goldens and `insta` `glob!`
   fixtures (one case per file in `core/tests/fixtures/`).
2. **Driver-trait tests** — `embedded-hal-mock` fakes an I²C bus to prove the
   seam future sensor drivers will plug into.
3. **Emulator smoke test** — Wokwi asserts the boot/heartbeat banner (see below).
4. **Hardware-in-the-loop** — occasional manual run on the real board, sanity
   checked against stock-firmware readings. Not in CI.

Snapshots run in **CI mode** under `CI=true` (and in GitHub Actions): a stale
snapshot **fails** rather than silently updating.

### Emulator (Wokwi)

`firmware/wokwi.toml` + `firmware/diagram.json` describe an ESP32-C3. With a
[Wokwi CI token](https://wokwi.com/dashboard/ci):

```sh
cargo build -p aq-firmware --target riscv32imc-unknown-none-elf --release
WOKWI_CLI_TOKEN=... wokwi-cli --timeout 15000 --expect-text "[aq] heartbeat #" firmware
```

CI runs this only when `WOKWI_CLI_TOKEN` is set in repo secrets, so it never blocks
token-less runs. Wokwi can't model the real sensors — it asserts serial/boot
behaviour only.

## CI

[`.github/workflows/ci.yml`](./.github/workflows/ci.yml) installs Nix and runs every
gate through the same dev shell: `cargo fmt --check`, `cargo clippy --all-targets -D
warnings` (host **and** the MCU target), `cargo test`, and the release firmware
build. The Wokwi smoke test is an optional follow-on job.

## Safety (read before flashing)

The device is effectively unbrickable (first-stage bootloader is in mask ROM), but
**NVS is not recoverable from public firmware**. Before flashing anything custom:

```sh
# Full-flash backup — the ONLY artifact preserving NVS (Wi-Fi creds, identity,
# factory calibration).
esptool read_flash 0 0x400000 stock-openair.bin
```

- **Restore** = full erase then full-image write, never an app-only reflash.
- **Never burn eFuses.** They are one-time-programmable — the single irreversible
  action on this chip. Nothing here needs them.

## Wedge roadmap

1. **Heartbeat** ✅ — boot + counted serial heartbeat (this wedge).
2. **Read one I²C sensor** — bring up a single sensor behind an `embedded-hal`
   trait; decode raw frames in `aq-core`.
3. **Corrections** — apply the PM/humidity corrections that make outdoor readings
   trustworthy, validated with fixture tests against stock-firmware behaviour.
4. **Wi-Fi** — join the network (introduces Embassy when concurrency is needed).
5. **MQTT → Home Assistant** — publish corrected readings.
