//! `aq-firmware`: the thin esp-hal adapter.
//!
//! All it does this wedge is bring up the chip and emit a counted heartbeat over
//! the C3's native USB-Serial-JTAG, once per second. The text itself is built by
//! [`aq_core::heartbeat_line`] so the wire format is host-tested; this file is
//! deliberately boring glue.
//!
//! Blocking only - no Embassy until a wedge actually needs concurrency.

#![no_std]
#![no_main]

use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::main;
use esp_println::println;

// Pulls in the panic handler, which prints a backtrace over the same
// jtag-serial console. RV32IMC has no native atomics, but esp-hal's `esp32c3`
// feature already enables `portable-atomic/unsafe-assume-single-core` (sound on
// this single-core chip), so we do not wire portable-atomic ourselves.
use esp_backtrace as _;

/// Heartbeat period. One second keeps the console lively without spamming it.
const HEARTBEAT_PERIOD_MS: u32 = 1000;

#[main]
fn main() -> ! {
    // Max clock: nothing here is power-sensitive yet, and it makes the console
    // feel responsive. `init` hands back the peripherals; we need none of them
    // for a bare heartbeat, so they stay unused for now.
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let _peripherals = esp_hal::init(config);

    let delay = Delay::new();

    let mut count: u32 = 0;
    loop {
        println!("{}", aq_core::heartbeat_line(count));
        // Saturating, not wrapping: after ~136 years of uptime the counter
        // simply parks at u32::MAX rather than reporting a bogus reboot.
        count = count.saturating_add(1);
        delay.delay_millis(HEARTBEAT_PERIOD_MS);
    }
}
