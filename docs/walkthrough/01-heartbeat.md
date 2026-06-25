---
chapter: 1
title: The heartbeat wedge
high_watermark: 0b89a5e
covers: <root>..0b89a5e
encoded: 2026-06-24
---

# Code walkthrough: the heartbeat wedge

A guided tour of everything in the repo as it stands after the first wedge, written
for someone learning **Rust** and **embedded systems** at the same time. It assumes
you can read basic Rust syntax but explains the embedded-specific and idiomatic bits
as they come up.

We'll go in the order that makes the *concepts* build on each other, not strictly
top-to-bottom by directory:

1. [The big picture: two crates, one repo](#1-the-big-picture-two-crates-one-repo)
2. [The pure core (`aq-core`)](#2-the-pure-core-aq-core)
3. [`no_std`, and why a string has a size](#3-no_std-and-why-a-string-has-a-size)
4. [The firmware adapter (`aq-firmware`)](#4-the-firmware-adapter-aq-firmware)
5. [How the build is wired for a bare-metal chip](#5-how-the-build-is-wired-for-a-bare-metal-chip)
6. [The test harness, layer by layer](#6-the-test-harness-layer-by-layer)
7. [The toolchain and dev environment (Nix)](#7-the-toolchain-and-dev-environment-nix)
8. [CI](#8-ci)
9. [Glossary](#9-glossary)

---

## 1. The big picture: two crates, one repo

The repo is a **Cargo workspace** (one repo, multiple packages sharing a lockfile and
`target/` dir). See [`Cargo.toml`](../../Cargo.toml):

```toml
[workspace]
resolver = "2"
members = ["core", "firmware"]
default-members = ["core"]
```

There are two member crates:

- **`core/`** → package **`aq-core`**: pure logic. No hardware, no OS, runs on your
  laptop under `cargo test`.
- **`firmware/`** → package **`aq-firmware`**: the actual program that runs on the
  microcontroller. It's a thin "adapter" that wires real hardware into `aq-core`.

This is the **hexagonal / ports-and-adapters** architecture. The valuable, bug-prone
logic (decoding sensor bytes, correction math) lives in `aq-core` where it's trivial
to test. The hardware-specific code stays in a thin shell around it. The project rule
is: *if logic can live in `aq-core`, it lives in `aq-core`.*

### Why `default-members`?

This is a subtle but important line. The firmware can only be compiled for the
microcontroller (a RISC-V chip), **not** for your laptop. If you ran a plain
`cargo test` and Cargo tried to build *every* member, it would try to compile the
firmware for your laptop and fail.

`default-members = ["core"]` means "when no specific package is named, only operate on
`core`." So:

- `cargo test` (at the repo root) → builds and tests **only `aq-core`**, on the host.
- `cargo build -p aq-firmware --target riscv32imc-unknown-none-elf` → explicitly
  builds the firmware for the chip.

The two targets coexist without stepping on each other. This "two targets, one repo"
split is the single trickiest bit of workspace setup here, and `default-members` is
what makes it painless.

---

## 2. The pure core (`aq-core`)

Open [`core/src/lib.rs`](../../core/src/lib.rs). It's small on purpose. The whole job of
this wedge's core is: *turn a tick counter into a line of text*.

```rust
pub fn format_uptime(seconds: u32) -> String<16> {
    let hours = seconds / SECS_PER_HOUR;
    let minutes = (seconds % SECS_PER_HOUR) / SECS_PER_MIN;
    let secs = seconds % SECS_PER_MIN;

    let mut out = String::new();
    let _ = write!(out, "{hours}:{minutes:02}:{secs:02}");
    out
}
```

A few Rust things worth pausing on:

- **`u32`** is an unsigned 32-bit integer. On embedded you're constantly choosing
  integer widths deliberately: `u32` is the native register width of this 32-bit chip,
  so it's the cheap, natural choice for a counter.
- **`{minutes:02}`** is a format specifier: pad to width 2 with leading zeros. So `5`
  becomes `05`. `{hours}` has no padding, which is deliberate (see the doc comment at
  [`core/src/lib.rs:20`](../../core/src/lib.rs#L20)): a board up for weeks prints
  `840:00:00`, not a misleading wrapped value.
- **`write!(out, ...)`** uses the `core::fmt::Write` trait (imported at
  [`core/src/lib.rs:13`](../../core/src/lib.rs#L13)). This is the same machinery as
  `println!`, but instead of writing to stdout it writes *into our string buffer*.
- **`let _ = write!(...)`** — `write!` returns a `Result` (it *can* fail if the buffer
  is full). `let _ =` explicitly discards it. We discard it because we proved by
  construction it can't fail here: a `String<16>` is big enough for any `u32` uptime.
  The comment at [`core/src/lib.rs:32`](../../core/src/lib.rs#L32) records *why*, which is
  the kind of "comment the why, not the what" the project asks for.

> **Rust idiom — the last expression is the return value.** `out` on its own line
> (no `return`, no semicolon) at [`core/src/lib.rs:35`](../../core/src/lib.rs#L35) *is*
> the return value. A trailing semicolon would turn it into a statement and break the
> return. This trips up everyone coming from C/Go at first.

The second function, [`heartbeat_line`](../../core/src/lib.rs#L43), composes on the first:

```rust
pub fn heartbeat_line(count: u32) -> String<48> {
    let mut out = String::new();
    let _ = write!(out, "[aq] heartbeat #{count} up {}", format_uptime(count));
    out
}
```

Note the design choice baked into the comment at
[`core/src/lib.rs:38`](../../core/src/lib.rs#L38): the firmware emits one heartbeat per
second, so `count` *is* the uptime in seconds. We encode that assumption **here, in
the testable core**, rather than in the hardware adapter. That's what lets us write a
test asserting the exact bytes that will hit the wire, without a chip in the loop.

---

## 3. `no_std`, and why a string has a size

The very top of the file, [`core/src/lib.rs:10-11`](../../core/src/lib.rs#L10):

```rust
#![no_std]
#![forbid(unsafe_code)]
```

### `#![no_std]`

Normal Rust programs link against **`std`**, the standard library, which assumes an
operating system underneath: a heap allocator, files, threads, networking, `println!`
to stdout. A microcontroller has **none of that** — there's no OS, a few hundred KB of
RAM, and no allocator unless you bring one.

`#![no_std]` opts out of `std` and links only against **`core`**: the subset of the
standard library that needs no OS and no heap (integers, slices, `Option`, `Result`,
iterators, the `fmt` machinery, etc.). The `#!` (with the bang) means the attribute
applies to the *whole crate*, not the next item.

This is *the* foundational embedded-Rust concept. Everything downstream — why we use a
fixed-size string, why dependencies must be carefully chosen — flows from "no heap, no
OS."

### `#![forbid(unsafe_code)]`

`unsafe` is Rust's escape hatch for operations the compiler can't verify (raw pointer
dereferences, etc.). `aq-core` is pure logic, so it has no business using `unsafe`.
`forbid` makes any `unsafe` block in this crate a hard **compile error**. It's a cheap
guardrail: the dangerous code is confined to the firmware adapter, never the core.

### Why `String<16>` instead of `String`?

This is the heap question made concrete. The `std` `String` grows on demand by
allocating heap memory. With `no_std` there's no heap, so we use
[`heapless::String`](../../core/Cargo.toml#L12) instead:

```rust
use heapless::String;        // not std::string::String
// ...
fn format_uptime(seconds: u32) -> String<16>
```

`String<16>` is a string with a **compile-time-fixed capacity of 16 bytes**, stored
inline (on the stack, or inside whatever struct holds it) — no allocation, ever. The
`16` is part of the type. If you try to push a 17th byte, the write fails gracefully
(returns an error) rather than growing.

This is the embedded trade-off in miniature: **you give up dynamic growth and in
return you get predictable, allocation-free memory use.** You'll see the same pattern
for vectors (`heapless::Vec<T, N>`) and queues. Choosing the capacity is a real design
decision — that's why the doc comment at [`core/src/lib.rs:24`](../../core/src/lib.rs#L24)
justifies why 16 is enough for any `u32`.

---

## 4. The firmware adapter (`aq-firmware`)

Now [`firmware/src/main.rs`](../../firmware/src/main.rs) — the code that actually runs on
the ESP32-C3. It's deliberately tiny.

### The two inner attributes

```rust
#![no_std]
#![no_main]
```

`#![no_std]` we've met. **`#![no_main]`** is new and embedded-specific. A normal Rust
program has a `fn main()` that the standard runtime calls *after* it sets up the
environment (stack, args, etc.). On bare metal there is no standard runtime to do that
setup, so we tell Rust "don't expect a conventional `main` entry point" — the real
entry point is established differently (next section).

### The entry point: `#[main]`

```rust
use esp_hal::main;

#[main]
fn main() -> ! {
```

This `#[main]` (no `!`, it's an *attribute macro* from `esp-hal`, not the function) is
the magic that replaces what `#![no_main]` removed. The macro expands to the real
reset-handler wiring the chip needs: it generates the symbol the CPU jumps to when it
powers on, sets up the stack, zeroes `.bss`, and then calls our function.

The return type **`-> !`** is the "never type": this function *never returns*. On a
microcontroller there's nothing to return *to* — no shell, no OS waiting for an exit
code. The program is the only thing running, so `main` must loop forever. The compiler
enforces this: if you could fall off the end of `main`, it wouldn't typecheck as `!`.
That's why the body ends in an infinite `loop {}`.

### Bringing up the chip

```rust
let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
let _peripherals = esp_hal::init(config);
```

- **`esp-hal`** is the **HAL** (Hardware Abstraction Layer) — a crate that wraps the
  chip's raw registers in safe Rust APIs. Without it you'd be writing magic numbers to
  memory addresses.
- `esp_hal::init` powers up the chip and hands back a `Peripherals` struct: a single
  value that *owns* every peripheral (every GPIO pin, every UART, the I²C controller,
  etc.). This is a gorgeous bit of Rust embedded design: because there's exactly one
  `Peripherals` and Rust's ownership rules forbid two mutable borrows of the same
  thing, **the type system prevents two parts of your code from grabbing the same pin
  at once.** Hardware conflicts become compile errors.
- We don't use any peripherals for a bare heartbeat, so we bind it to `_peripherals`.
  The leading underscore tells the compiler "intentionally unused, don't warn." (A
  plain unused `peripherals` would trip `clippy`'s `-D warnings` in CI.)

### The `_ = ...` panic-handler import

```rust
use esp_backtrace as _;
```

This imports the [`esp-backtrace`](../../firmware/Cargo.toml#L11) crate **for its side
effects only** — the `as _` means "I don't want a name for this, I just need it
linked." Why link a crate you never call?

Because `no_std` has no default **panic handler**. In `std`, when code panics
(e.g. an array index out of bounds), the runtime prints a message and unwinds. With
`no_std` you must *provide* the function that runs on panic, or the program won't even
link. `esp-backtrace` provides one: on panic it prints a stack backtrace over the
serial console. Just linking it satisfies the `#[panic_handler]` requirement. The
comment at [`firmware/src/main.rs:18`](../../firmware/src/main.rs#L18) also notes it
quietly settles the atomics story (more in §5).

### The loop

```rust
let mut count: u32 = 0;
loop {
    println!("{}", aq_core::heartbeat_line(count));
    count = count.saturating_add(1);
    delay.delay_millis(HEARTBEAT_PERIOD_MS);
}
```

- **`println!`** here is from [`esp-println`](../../firmware/Cargo.toml#L13), **not**
  `std`. It routes the text over the chip's USB-Serial-JTAG console (see §5). Same
  familiar macro surface, totally different plumbing underneath.
- All the actual *formatting* is delegated to `aq_core::heartbeat_line(count)` — the
  host-tested function from §2. This is the "thin adapter" principle in one line: the
  firmware decides *when* and *where* to print; the core decides *what* the text is.
- **`count.saturating_add(1)`** — instead of plain `count + 1`, this *saturates* at
  `u32::MAX` rather than wrapping around to 0. In release builds Rust doesn't check for
  overflow by default (it wraps silently), so being explicit about the overflow policy
  is good embedded hygiene. The comment explains the real-world reasoning: a wrap would
  look like a spurious reboot in the logs.
- **`delay.delay_millis(1000)`** is a **busy-wait**: the CPU spins for ~1 second doing
  nothing. That's fine for a heartbeat but wasteful in general — burning a whole CPU to
  wait is exactly the problem `async`/Embassy solves later, when we need to do other
  work while waiting. We deliberately *don't* reach for that complexity yet.

> **Why blocking, not async, yet?** The project builds in "wedges" and adds complexity
> only when a wedge needs it. A single heartbeat has no concurrency, so a blocking loop
> is the honest, simplest thing. Embassy (async embedded) arrives when we're juggling
> Wi-Fi *and* sensor reads *and* publishing at once.

---

## 5. How the build is wired for a bare-metal chip

This is the part that's pure embedded-systems plumbing and has no host-Rust equivalent.
Four files cooperate.

### The target triple

The firmware is built for **`riscv32imc-unknown-none-elf`**. Read it as four parts:

- `riscv32` — 32-bit RISC-V CPU architecture.
- `imc` — the ISA extensions present: **I**nteger, **M**ultiply/divide, **C**ompressed
  instructions. Notably **no `a`** (atomics) — that absence drives a real decision below.
- `unknown` — vendor unspecified.
- `none` — **no operating system**. This is the part that implies `no_std`.
- `elf` — the output binary format.

### Where the target is set: `firmware/.cargo/config.toml`

[`firmware/.cargo/config.toml`](../../firmware/.cargo/config.toml):

```toml
[build]
target = "riscv32imc-unknown-none-elf"

[target.riscv32imc-unknown-none-elf]
runner = "espflash flash --monitor"
```

Cargo reads `.cargo/config.toml` files based on **where you run the command**, walking
up from the current directory. So this file only takes effect when you run Cargo from
inside `firmware/`. That's deliberate:

- `cd firmware && cargo run` → builds for the chip (`[build] target`), and `runner`
  redefines what "run" *means*: instead of executing the binary locally (impossible —
  it's RISC-V), it invokes **`espflash`** to flash the binary onto the board over USB
  and then opens a serial monitor. That's how `cargo run` flashes hardware.
- From the repo root, this file is *not* read, so the host stays the default — which is
  why `cargo test` at the root Just Works.

### The link argument: repo-root `.cargo/config.toml`

[`.cargo/config.toml`](../../.cargo/config.toml) at the repo root:

```toml
[target.riscv32imc-unknown-none-elf]
rustflags = [
  "-C", "link-arg=-Tlinkall.x",
  "-C", "force-frame-pointers",
]
```

This is keyed by the **target triple**, not by `[build]`, so it applies *whenever that
target is selected* and **never** for the host. That distinction matters: it's why we
can keep MCU-specific flags at the root (shared by both `cd firmware && cargo run` and
`cargo build -p aq-firmware --target ...`) without ever forcing the host build onto the
MCU target.

What the flags do:

- **`-Tlinkall.x`** points the linker at a **linker script**. A linker script is the
  memory map: it tells the linker *where* in the chip's address space each section of
  your program goes — code into flash, data into RAM, the interrupt vector table at the
  exact address the CPU expects, etc. A normal hosted program never needs this because
  the OS loader handles placement; bare metal, *you* specify it. `linkall.x` is an
  umbrella script `esp-hal` ships; it pulls in the chip's memory layout and the
  interrupt vector table. (Bringing this up was the one genuinely fiddly part of getting
  the firmware to link — without it the linker can't resolve the interrupt vectors.)
- **`force-frame-pointers`** keeps a frame pointer in every function so
  `esp-backtrace` can walk the stack and print a useful backtrace on panic.

### The atomics decision (the `imc` "no `a`" payoff)

Recall the target has no atomics extension. Plenty of library code assumes atomic
operations exist (for safe sharing across execution contexts). On this chip the
hardware can't do them directly. The fix is the
[`portable-atomic`](https://docs.rs/portable-atomic) crate, which *emulates* atomics.
On a **single-core** chip you can emulate an atomic by briefly disabling interrupts —
sound precisely because nothing else can run concurrently.

Here's the nice part: we **don't** wire this ourselves. `esp-hal`'s `esp32c3` feature
already turns on `portable-atomic/unsafe-assume-single-core` for us. We discovered
(the hard way) that *also* enabling `portable-atomic`'s `critical-section` feature
**conflicts** with that and fails to compile. So the firmware's
[`Cargo.toml`](../../firmware/Cargo.toml) deliberately does *not* depend on
`portable-atomic` at all — the comment at
[`firmware/src/main.rs:18`](../../firmware/src/main.rs#L18) records the reasoning. This is
a great example of an embedded "sharp edge": the constraint is real (no hardware
atomics), but the modern HAL has already paved over it, and the mistake is *over*-wiring.

### The console is the USB cable (and is coupled to your code)

One ESP32-C3 quirk worth internalizing: the chip has a **native USB-Serial-JTAG**
peripheral, so the single USB-C cable carries power, flashing, the `println!` console,
*and* debugging — no separate USB-to-serial chip. Consequences you'll feel:

- The console **re-enumerates on every reset** (it's part of the chip that resets), so
  your serial monitor drops and reconnects each boot. Normal, not a bug.
- The console is **coupled to your firmware being alive.** If the firmware hangs or
  panics badly, it can take the console down with it. (This is part of why a
  panic handler that prints a backtrace, §4, is valuable.)

Also note: both of the chip's *hardware UARTs* are reserved for sensors, which is
exactly why logging must go over USB-Serial-JTAG. That's the
[`jtag-serial` feature](../../firmware/Cargo.toml#L13) on `esp-println` — it selects the
USB-Serial-JTAG backend rather than a UART backend.

---

## 6. The test harness, layer by layer

The philosophy: **test at the cheapest layer that can cover the behaviour.** Three
layers exist today, all running on the host with a plain `cargo test`. Each
demonstrates a different testing tool you'll reuse for real sensor logic later.

### Layer 1a: inline golden tests (`expect-test`)

At the bottom of [`core/src/lib.rs:49-70`](../../core/src/lib.rs#L49):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    #[test]
    fn uptime_renders_hms() {
        expect!["1:01:05"].assert_eq(format_uptime(3665).as_str());
    }
}
```

- **`#[cfg(test)]`** means "only compile this module when running tests" — it's not in
  the shipped firmware.
- **`#[test]`** marks a test function, same as everywhere in Rust.
- **`expect!["..."]`** holds a *golden value* — the expected output, written inline.
  The magic: if you change the format and the output changes, you don't hand-edit the
  expected strings. You run `UPDATE_EXPECT=1 cargo test` and the tool **rewrites the
  literals in the source file** for you. You then read the diff to confirm the change
  was intended. It's a fast, low-ceremony way to keep expectations in sync with code.

This layer is for small, in-file expectations right next to the function.

### Layer 1b: file-driven snapshot tests (`insta` + `glob!`)

[`core/tests/heartbeat_snapshots.rs`](../../core/tests/heartbeat_snapshots.rs):

```rust
#[test]
fn heartbeat_lines() {
    glob!("fixtures/*.txt", |path| {
        let raw = fs::read_to_string(path).expect("fixture readable");
        let count: u32 = raw.trim().parse().expect("fixture is a u32 tick count");
        // ...
        assert_snapshot!(aq_core::heartbeat_line(count).as_str());
    });
}
```

Anything in `core/tests/` is an **integration test**: it's a separate crate that uses
`aq-core` as an external dependency (note it calls `aq_core::heartbeat_line`, going
through the public API, not `super::`). This is distinct from the `#[cfg(test)]` module
*inside* `lib.rs`, which can see private items.

The pattern here is **one test case per input file**:

- [`core/tests/fixtures/`](../../core/tests/fixtures/) holds three input files —
  `00-boot.txt` (`0`), `01-one-hour.txt` (`3600`), `02-u32-max.txt` (`4294967295`).
- `glob!` runs the closure once per matching file.
- `assert_snapshot!` compares the output against a stored snapshot in
  [`core/tests/snapshots/`](../../core/tests/snapshots/). For example
  `...@01-one-hour.snap` contains `[aq] heartbeat #3600 up 1:00:00`.

To add a test case you drop in a new `.txt` file and run `cargo insta review` to accept
the generated snapshot — no test code changes. This is ideal for **table-shaped logic**:
exactly what sensor decoding (raw bytes → struct) and correction math (conditions →
corrected value) will be. We're proving the harness now, on trivial logic, so it's
ready when the logic gets hairy.

> **A real gotcha we hit:** snapshots are committed to git and CI runs in "no-update"
> mode (`CI=true`), so a *stale* snapshot **fails the build** instead of being silently
> rewritten. That's the point — it makes an unintended output change a visible failure.
> If you change a format, regenerate locally and commit the new `.snap` files.

### Layer 2: driver tests against traits (`embedded-hal-mock`)

[`core/tests/driver_trait.rs`](../../core/tests/driver_trait.rs) is the most
embedded-flavoured test, and it's reserving a seam for the future:

```rust
fn read_register<I: I2c>(i2c: &mut I, addr: u8, reg: u8) -> Result<u8, I::Error> {
    let mut buf = [0u8; 1];
    i2c.write_read(addr, &[reg], &mut buf)?;
    Ok(buf[0])
}
```

The key idea is **generics over a trait**. `read_register` doesn't take a *specific*
I²C peripheral; it takes any type `I` that implements the
[`embedded_hal::i2c::I2c`](../../core/Cargo.toml#L15) trait. **`embedded-hal`** is the
standard set of traits (`I2c`, `SpiBus`, `DelayNs`, ...) that every chip's HAL
implements. Because real drivers are written against these traits, you can run the
*same* driver code against:

- the real `esp-hal` I²C peripheral on the chip, and
- a **mock** bus on your laptop in a test.

The test uses the mock:

```rust
let expectations = [Transaction::write_read(addr, vec![0x00], vec![0xAB])];
let mut i2c = I2cMock::new(&expectations);

let value = read_register(&mut i2c, addr, 0x00).expect("mock transaction");
assert_eq!(value, 0xAB);

i2c.done();
```

You *script* the bus: "expect a write-read of register `0x00`, reply with byte `0xAB`."
Then `i2c.done()` asserts every scripted transaction actually happened. This is how
you'll test a real sensor driver — its exact byte-level conversation with the chip —
**without any hardware**, deterministically, in CI. The `read_register` "driver" here
is a stub whose only job is to prove the seam compiles and runs.

> **The `?` operator:** `i2c.write_read(...)?` — the trailing `?` means "if this
> returned an `Err`, return it from the enclosing function immediately; otherwise
> unwrap the `Ok`." It's Rust's concise error-propagation. The function returns
> `Result<u8, I::Error>`, so an error bubbles up to the caller rather than panicking.
> On embedded, returning typed `Result`s (rather than crashing) is the norm for
> anything recoverable.

### The layers we don't run in CI

Two more layers exist in principle: a **Wokwi emulator** smoke test (§8) that boots the
real firmware binary in a simulator and asserts the heartbeat banner appears, and
**hardware-in-the-loop** (flashing a real board and eyeballing it against the stock
firmware). The first is optional in CI; the second is manual.

---

## 7. The toolchain and dev environment (Nix)

There's no global Rust install on this machine; the entire toolkit comes from a **Nix
flake** ([`flake.nix`](../../flake.nix)). Think of it as a reproducible, project-scoped
box of tools: the exact Rust compiler, plus `espflash`, `esptool`, and `cargo-insta`,
identical on every machine and in CI.

```nix
rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
```

The clever bit: the flake reads [`rust-toolchain.toml`](../../rust-toolchain.toml) — the
*same* file `rustup` would use — so there's exactly **one** definition of the toolchain
(stable channel + the `riscv32imc-unknown-none-elf` target + `clippy`/`rustfmt`),
shared by Nix, any local `rustup`, and CI. No drift.

You enter the environment with `nix develop` (which puts all those tools on your
`PATH`), or prefix a single command: `nix develop --command cargo test`. Two practical
notes captured in [`CLAUDE.md`](../../CLAUDE.md):

- **Flakes only see git-tracked files.** A brand-new file is invisible to `nix develop`
  until you `git add` it. (We hit exactly this on the first run.)
- Adding a project tool means adding it to the `packages` list in `flake.nix`, never a
  global install — that's the whole point.

`rust-toolchain.toml` itself uses `profile = "minimal"` to keep the download small,
then explicitly adds back `clippy` and `rustfmt` (the linter and formatter, both
required by CI).

---

## 8. CI

[`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) runs every gate through the
*same* Nix shell, so "passes locally" and "passes CI" mean the same thing. The gates,
each a hard requirement:

- `cargo fmt --all --check` — formatting is correct (fails if not, doesn't fix).
- `cargo clippy --all-targets -- -D warnings` — the linter, with **warnings escalated
  to errors** (`-D warnings`). Run for the host *and* separately for the MCU target.
- `cargo test` — the host test layers from §6. GitHub sets `CI=true`, which flips
  `insta` into the "stale snapshot fails" mode.
- `cargo build -p aq-firmware --target riscv32imc-unknown-none-elf --release` — the
  firmware actually compiles for the chip.

A second job runs the optional **Wokwi** emulator smoke test, but only if a
`WOKWI_CLI_TOKEN` secret is configured — so it never blocks contributors who don't have
one. It builds the firmware and asserts the string `[aq] heartbeat #` shows up on the
simulated serial console: proof the binary boots and the loop runs, without real
hardware.

---

## 9. Glossary

| Term | What it means here |
|---|---|
| **`no_std`** | Build without the standard library (no OS, no heap). Links only `core`. |
| **`core`** (the crate) | The always-available subset of std: integers, `Result`, `fmt`, slices. Not to be confused with our `core/` directory (package `aq-core`). |
| **HAL** | Hardware Abstraction Layer — safe Rust wrappers over chip registers (`esp-hal`). |
| **`embedded-hal`** | Vendor-neutral *traits* (`I2c`, `SpiBus`, ...) every HAL implements; lets drivers be hardware-agnostic and host-testable. |
| **PAC** | Peripheral Access Crate — the lowest-level, register-by-register chip definitions the HAL is built on (here, the `esp32c3` crate). |
| **target triple** | The `arch-vendor-os-abi` string naming what you compile for (`riscv32imc-unknown-none-elf`). |
| **linker script** | The memory map telling the linker where code/data/vectors go in the chip's address space (`linkall.x`). |
| **panic handler** | The function that runs when Rust panics; mandatory and self-supplied in `no_std` (`esp-backtrace`). |
| **`#[main]` / `#![no_main]`** | The macro that generates the bare-metal entry point, replacing the standard `main` runtime. |
| **`-> !`** | The "never type": a function that never returns (the firmware loops forever). |
| **atomics / `portable-atomic`** | Operations for safe concurrent access; absent in this chip's ISA, emulated via single-core interrupt masking. |
| **USB-Serial-JTAG** | The C3's built-in USB peripheral carrying power + flashing + console + debug on one cable. |
| **busy-wait / blocking** | Holding the CPU in a spin loop to wait (`delay_millis`); the thing async/Embassy later avoids. |
| **wedge** | This project's unit of work: one small, reviewable, tested step toward the full firmware. |

---

### Where to go next

The natural next wedge (see the roadmap in the [README](../../README.md)) is **reading one
real I²C sensor**. When you get there, the shape is already laid out: write the driver
generic over `embedded_hal::i2c::I2c` (like the stub in §6), put the byte-decoding logic
in `aq-core` behind `insta` fixture tests, and let `aq-firmware` do nothing but hand the
real `esp-hal` I²C bus to that code. Almost everything stays a host `cargo test`.
