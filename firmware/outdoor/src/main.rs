//! `aq-outdoor`: the AirGradient Open Air O-1PST firmware entry point.
//!
//! A stub for now (no outdoor unit in hand): it boots and beats exactly like the
//! indoor binary, differing only in its board profile. It exists so the
//! multi-board structure stays exercised - both binaries build in CI - and so the
//! outdoor sensor path has a home to grow into. Everything interesting lives in
//! [`aq_adapter`]; the indoor counterpart is `aq-indoor`.

#![no_std]
#![no_main]

use esp_hal::main;

// Pulls in the panic handler, which prints a backtrace over the same jtag-serial
// console. Must be linked by the binary, not the adapter library.
use esp_backtrace as _;

// Embed the ESP-IDF application descriptor. The ESP32-C3 second-stage bootloader
// (and Wokwi) refuses to boot an app image that lacks it: without it control never
// reaches `main`, so nothing ever prints. Must live in the binary crate.
esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    aq_adapter::run(aq_adapter::profiles::OUTDOOR)
}
