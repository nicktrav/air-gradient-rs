//! `aq-core`: the pure, `no_std`, host-testable heart of the firmware.
//!
//! Everything that *can* be plain data-in/data-out logic lives here so it can be
//! exercised with a normal `cargo test` on the host. The firmware binary is a
//! thin adapter that feeds real peripheral data into these functions.
//!
//! This first wedge only needs to turn a tick counter into console text; sensor
//! decode and correction math will land here behind the same testing idioms.

#![no_std]
#![forbid(unsafe_code)]

use core::fmt::Write;
use heapless::String;

/// Seconds in a minute / minute in an hour, named so the math below reads.
const SECS_PER_MIN: u32 = 60;
const SECS_PER_HOUR: u32 = 60 * SECS_PER_MIN;

/// Format a whole-second uptime as `HH:MM:SS`.
///
/// Hours are not clamped to two digits: a board left running for weeks should
/// still report honestly (e.g. `840:00:00`), so the hour field simply grows.
/// The capacity (16) comfortably holds the widest `u32`-seconds value
/// (`1193046:28:15`).
pub fn format_uptime(seconds: u32) -> String<16> {
    let hours = seconds / SECS_PER_HOUR;
    let minutes = (seconds % SECS_PER_HOUR) / SECS_PER_MIN;
    let secs = seconds % SECS_PER_MIN;

    let mut out = String::new();
    // Writing into a String<16> can only fail by overflowing capacity, which the
    // bound above rules out for any u32; the error is therefore unreachable.
    let _ = write!(out, "{hours}:{minutes:02}:{secs:02}");
    out
}

/// Build the heartbeat console line for a given board and heartbeat count.
///
/// `board` is the per-board tag (e.g. `aq-indoor`, `aq-outdoor`) so the two
/// firmware artifacts emit distinguishable lines from one shared formatter; the
/// adapter passes its [`BoardProfile`] name straight through. The firmware emits
/// one heartbeat per second, so `count` doubles as an uptime in seconds. Keeping
/// both assumptions here (rather than in the adapter) is what lets us
/// snapshot-test the exact bytes that hit the wire.
///
/// Capacity 64 holds the widest line: the longest board tag we use plus the
/// widest `u32` count and uptime (`[aq-outdoor] heartbeat #4294967295 up
/// 1193046:28:15`).
pub fn heartbeat_line(board: &str, count: u32) -> String<64> {
    let mut out = String::new();
    let _ = write!(
        out,
        "[{board}] heartbeat #{count} up {}",
        format_uptime(count)
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    // Inline golden test: run `UPDATE_EXPECT=1 cargo test` to rewrite the
    // expected literal in place after changing the format.
    #[test]
    fn uptime_renders_hms() {
        expect!["1:01:05"].assert_eq(format_uptime(3665).as_str());
    }

    #[test]
    fn uptime_zero() {
        expect!["0:00:00"].assert_eq(format_uptime(0).as_str());
    }

    #[test]
    fn uptime_hours_grow_past_two_digits() {
        expect!["100:00:00"].assert_eq(format_uptime(360_000).as_str());
    }

    // The board tag is what distinguishes the two firmware artifacts on the wire,
    // so prove the same count renders a different line per board.
    #[test]
    fn heartbeat_tags_the_board() {
        expect!["[aq-indoor] heartbeat #7 up 0:00:07"]
            .assert_eq(heartbeat_line("aq-indoor", 7).as_str());
        expect!["[aq-outdoor] heartbeat #7 up 0:00:07"]
            .assert_eq(heartbeat_line("aq-outdoor", 7).as_str());
    }
}
