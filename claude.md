# `air-gradient-rs`

## Project

Custom `no_std` Rust firmware for an **AirGradient Open Air O-1PST** outdoor air
quality monitor (**ESP32-C3-MINI**, RISC-V). This is a **learning project for
embedded systems** — the journey is a goal in itself, so we favour clarity,
correctness, and understanding over shortcuts or magic.

The work is built in small **wedges**. The endgame is: read the on-board sensors
(particulate matter, CO₂, temperature/humidity), apply the corrections that make
outdoor readings trustworthy, and publish over Wi-Fi/MQTT to Home Assistant. We
get there one reviewable, tested step at a time — not in one big firmware drop.

## Hardware facts that constrain the code

- **MCU:** ESP32-C3-MINI, RISC-V `RV32IMC`. Target `riscv32imc-unknown-none-elf`,
  `no_std`, **mainline stable Rust** — no `espup`, no Xtensa toolchain fork.
- **`RV32IMC` has no native atomics** (`a` extension absent). On esp-hal 1.x this
  is already handled: the `esp32c3` feature pulls in
  `portable-atomic/unsafe-assume-single-core` (sound on this single-core part). Do
  **not** also wire `portable-atomic`'s `critical-section` impl — the two conflict.
  See "ESP32-C3 build wiring" below.
- **Console:** the C3's **native USB-Serial-JTAG** drives the USB-C port directly
  — no CH340/CP2102 bridge chip. One cable carries power + flashing + `println!`
  console + JTAG debug. Consequence: the console is **coupled to app liveness**
  (a hung/panicked firmware can take the console with it) and **re-enumerates on
  every reset**, so the monitor drops and reconnects each boot.
- **Both hardware UARTs are consumed by sensors.** Logging therefore **must** use
  the `jtag-serial` backend of `esp-println` (or `defmt` over it), never a UART
  backend. There is no spare UART for a traditional console.
- **Sensors** hang off I²C/UART (particulate, CO₂ NDIR, temp/humidity). Do **not**
  hard-code a sensor lineup from memory — confirm part numbers against the actual
  unit, and treat **stock AirGradient firmware as the ground-truth reference** for
  what a given sensor *should* read.

## Hardware-ops safety rules (non-negotiable)

- **Back up first.** Before flashing anything custom, dump the full flash:
  `esptool read_flash 0 0x400000 stock-openair.bin`. This is the only artifact that
  preserves the NVS partition (Wi-Fi creds, device identity, factory calibration);
  the public firmware can't restore those.
- **Restore = full erase then full-image write**, never an app-only reflash. An app
  dropped on top of mismatched NVS boots stale and looks like a code bug.
- **Never burn eFuses.** They are one-time-programmable and the single irreversible
  action on this chip. Nothing in this project requires them.
- Otherwise the device is **effectively unbrickable** — the first-stage bootloader
  is in mask ROM and always accepts a fresh flash via download mode.

## Toolkit

| Purpose            | Tool / crate                                              |
|--------------------|-----------------------------------------------------------|
| Toolchain          | stable Rust + `rustup target add riscv32imc-unknown-none-elf` |
| HAL                | `esp-hal` (1.0-beta line); `embedded-hal[-async]` traits  |
| Async (later)      | `embassy` — adopt only when real concurrency is needed    |
| Scaffold           | `esp-generate`                                            |
| Flash / monitor    | `espflash` (wired as the cargo `runner`; `cargo run`)     |
| Flash backup       | `esptool`                                                 |
| Logging            | `esp-println` / `defmt`, **`jtag-serial` backend**        |
| Host testing       | `expect-test`, `insta` (+ `cargo-insta`)                  |
| Driver testing     | `embedded-hal-mock`                                       |
| Emulator           | **Wokwi** (`wokwi-cli`); QEMU (Espressif fork) as fallback |

Don't pin exact versions in this doc — let `cargo add` resolve current, and pin the
toolchain via `rust-toolchain.toml`.

## Development environment (Nix)

The whole toolkit is provided by a **Nix flake** — there is no global `rustup`
install to drift. `flake.nix` uses the `oxalica/rust-overlay` and reads
`rust-toolchain.toml`, so the flake, a local `rustup`, and CI all resolve **one**
toolchain (channel + `riscv32imc` target + `clippy`/`rustfmt`).

- **Enter the shell:** `nix develop` (or set up `direnv` with `use flake`). This puts
  `cargo`, `rustc`, `clippy`, `rustfmt`, `espflash`, `esptool`, and `cargo-insta` on
  `PATH`.
- **Run everything through it.** Either drop into `nix develop` once, or prefix a
  command: `nix develop --command cargo test`.
- **Flakes only see git-tracked files** — `git add` new files before `nix develop`
  picks them up, or Nix errors with "not tracked by Git".
- **CI uses the same shell** (`DeterminateSystems/nix-installer-action` +
  `nix develop --command ...`), so "works in the shell" means "works in CI".

Adding a host tool the project needs (e.g. `probe-rs`, `wokwi-cli` if packaged)?
Add it to the `devShells.default` package list in `flake.nix`, not to a global profile.

### ESP32-C3 build wiring (learned bring-up facts)

- **`esp-hal` resolved to the 1.x stable line** (not beta). Most of the HAL beyond
  bare init (e.g. `esp_hal::delay::Delay`) is gated behind its **`unstable`** feature —
  enable `esp-hal/unstable` alongside the `esp32c3` chip feature.
- **Atomics are already handled.** esp-hal's `esp32c3` feature enables
  `portable-atomic/unsafe-assume-single-core` (sound: the C3 is single-core). Do **not**
  also add `portable-atomic` with its `critical-section` feature — the two conflict at
  compile time. (This supersedes the older "wire portable-atomic/critical-section" note.)
- **Linker script:** the firmware target needs `-C link-arg=-Tlinkall.x`
  (esp-hal's umbrella script, which pulls in the PAC `device.x` interrupt vectors).
  It lives in the **repo-root** `.cargo/config.toml` keyed by the MCU triple so both
  `cd firmware && cargo run` and root-level `cargo build -p aq-firmware --target …`
  get it, without making the host build see an MCU default target.

## Architecture idiom: testable core, thin adapter

Hexagonal / ports-and-adapters. The workspace splits in two:

```
/                         workspace root (default = host target)
├── CLAUDE.md
├── rust-toolchain.toml   pinned toolchain
├── core/                 package: aq-core   (#![no_std], pure, host-testable)
│   ├── src/lib.rs        sensor-frame decode, PM/humidity correction, AQI,
│   │                     MQTT payload building, state machines
│   └── tests/
│       └── fixtures/     data-driven test cases
├── firmware/             package: aq-firmware (#![no_std] bin, builds for MCU)
│   ├── src/main.rs       thin adapter: esp-hal peripherals -> aq-core via traits
│   ├── .cargo/config.toml  target + espflash runner (scoped to this crate)
│   ├── wokwi.toml
│   └── diagram.json
└── .github/workflows/ci.yml
```

**Rule:** if logic *can* live in `aq-core`, it lives in `aq-core`. `aq-core`
depends only on `embedded-hal` traits for any I/O it abstracts — **never `esp-hal`**.
The `firmware` binary is boring glue. This is what makes nearly everything a plain
host `cargo test`. (Package is named `aq-core`, not `core` — never shadow the std
`core` crate name.)

## Testing idioms — test always

Cheapest layer first; new behaviour lands with a test at the lowest layer that can
cover it; bug fixes land with a **regression fixture first**.

1. **Host unit + data-driven tests on `aq-core`** (`cargo test`, runs on host).
   This is our **cockroachdb/datadriven analog**:
   - `expect-test` for inline golden values — `UPDATE_EXPECT=1 cargo test` rewrites
     them, mirroring datadriven's `-rewrite`.
   - `insta` with `glob!` to run one case per file in `core/tests/fixtures/`;
     `cargo insta review` to accept diffs.
   - Point the **fixture-file** style at table-shaped logic: sensor decode (raw
     bytes in → decoded struct out) and correction math (conditions in → corrected
     value out).
2. **Driver tests against `embedded-hal` traits** using `embedded-hal-mock` (fake
   I²C/SPI/UART) — still host, no emulator.
3. **Emulator smoke test:** build the firmware, run under **Wokwi** via
   `wokwi-cli --expect-text "..."` to assert boot + heartbeat. Wokwi won't model
   every sensor, so assert serial/boot behaviour, not sensor fidelity.
4. **Hardware-in-the-loop:** occasional manual run on the real board, sanity-checked
   against stock-firmware readings. **Not** in CI.

**Determinism:** no wall-clock or network in `aq-core`. Inject time and inputs so
every test is reproducible.

## CI — every commit builds and tests clean

GitHub Actions. All gates required; any failure blocks merge:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test` (host: `aq-core` + `embedded-hal-mock` driver tests)
- `cargo build -p aq-firmware --target riscv32imc-unknown-none-elf --release`
- *(optional, once stable)* `wokwi-cli` smoke test on the built ELF
  (needs `WOKWI_CLI_TOKEN` in repo secrets)
- Snapshots run in **CI mode** so stale ones **fail** — never auto-update in CI
  (don't pass `UPDATE_EXPECT`; `insta` respects the `CI` env var).

Toolchain pinned via `rust-toolchain.toml` for reproducible builds.

## Workspace gotcha: two targets, one repo

Keep the embedded target **scoped to `firmware/`** via its own
`firmware/.cargo/config.toml`:

```toml
[build]
target = "riscv32imc-unknown-none-elf"

[target.riscv32imc-unknown-none-elf]
runner = "espflash flash --monitor"
```

Run host tests from the **repo root** (default host target). Do **not** set a global
default target, or `cargo test` will try to build `aq-core` for the MCU and the host
test harness won't link.

## Conventions

- Small, reviewable commits; each one leaves CI green.
- `aq-core` returns `Result` with typed errors; panics only at adapter edges where a
  fault is genuinely unrecoverable.
- Comment the **why** — hardware quirks, datasheet section refs — not the *what*.
- Don't reach for Embassy, Wi-Fi, MQTT, or real sensor drivers before the wedge that
  needs them. Prove the harness around a trivial heartbeat first.
