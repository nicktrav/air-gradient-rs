//! `aq-adapter`: the shared esp-hal glue both board binaries run on.
//!
//! Both AirGradient units (the indoor ONE / I-9PSL and the outdoor Open Air
//! O-1PST) are the same ESP32-C3, so the chip bring-up and the heartbeat loop are
//! identical; only the [`BoardProfile`] differs. Keeping that glue here (instead
//! of duplicated in each `aq-indoor` / `aq-outdoor` binary) is what makes "one
//! crate per board" not mean "copy-pasted firmware".
//!
//! This crate is a library, so it does NOT define the panic handler or embed the
//! ESP-IDF app descriptor - those are linked by the final binary. Each board bin
//! pulls in `esp-backtrace` and calls `esp_app_desc!()`, then hands control here.
//!
//! Blocking only - no Embassy until a wedge actually needs concurrency.

#![no_std]

use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_println::println;

/// What distinguishes one board's firmware from the other.
///
/// Today this is just the console tag and the heartbeat period. It is the growth
/// point for the real per-board divergence: the sensor lineup and the
/// indoor-vs-outdoor correction selector land here as those wedges arrive. The
/// exact sensor variants stay out of this struct until the stock firmware's
/// `[Configure] Info` config confirms them (see CLAUDE.md), so we don't bake a
/// spec-sheet guess into code.
pub struct BoardProfile {
    /// Console tag, e.g. `aq-indoor`. Passed straight to
    /// [`aq_core::heartbeat_line`] so each artifact's output is identifiable.
    pub name: &'static str,
    /// Heartbeat period in milliseconds.
    pub heartbeat_period_ms: u32,
}

/// The two board profiles. Each board binary imports exactly one.
pub mod profiles {
    use super::BoardProfile;

    /// AirGradient ONE (I-9PSL), indoor. The primary, actively-built board.
    pub const INDOOR: BoardProfile = BoardProfile {
        name: "aq-indoor",
        heartbeat_period_ms: 1000,
    };

    /// AirGradient Open Air O-1PST, outdoor. A stub for now: it boots and beats
    /// like the indoor board but grows its own sensor path once that unit is in
    /// hand.
    pub const OUTDOOR: BoardProfile = BoardProfile {
        name: "aq-outdoor",
        heartbeat_period_ms: 1000,
    };
}

/// Bring up the chip and run the board's heartbeat loop forever.
///
/// All it does this wedge is emit a counted, board-tagged heartbeat over the C3's
/// native USB-Serial-JTAG once per period. The text itself is built by
/// [`aq_core::heartbeat_line`] so the wire format stays host-tested; this is
/// deliberately boring glue.
pub fn run(profile: BoardProfile) -> ! {
    // Max clock: nothing here is power-sensitive yet, and it makes the console
    // feel responsive. `init` hands back the peripherals; we need none of them for
    // a bare heartbeat, so they stay unused for now.
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let _peripherals = esp_hal::init(config);

    let delay = Delay::new();

    let mut count: u32 = 0;
    loop {
        println!("{}", aq_core::heartbeat_line(profile.name, count));
        // Saturating, not wrapping: after ~136 years of uptime the counter simply
        // parks at u32::MAX rather than reporting a bogus reboot.
        count = count.saturating_add(1);
        delay.delay_millis(profile.heartbeat_period_ms);
    }
}
