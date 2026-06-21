//! Driver-trait test: proves the `embedded-hal` + `embedded-hal-mock` seam
//! compiles and runs on the host, with no emulator and no real hardware.
//!
//! There is no real driver yet. This stands in for the shape every future
//! sensor driver will take: a function generic over an `embedded_hal` bus,
//! verified here against a mock that asserts the exact transactions.

use embedded_hal::i2c::I2c;
use embedded_hal_mock::eh1::i2c::{Mock as I2cMock, Transaction};

/// Stub "driver": read one byte from a register over I2C. Generic over the bus
/// so the same code runs against a mock here and a real esp-hal I2C later.
fn read_register<I: I2c>(i2c: &mut I, addr: u8, reg: u8) -> Result<u8, I::Error> {
    let mut buf = [0u8; 1];
    i2c.write_read(addr, &[reg], &mut buf)?;
    Ok(buf[0])
}

#[test]
fn reads_a_register_over_mock_i2c() {
    let addr = 0x42;
    let expectations = [Transaction::write_read(addr, vec![0x00], vec![0xAB])];
    let mut i2c = I2cMock::new(&expectations);

    let value = read_register(&mut i2c, addr, 0x00).expect("mock transaction");
    assert_eq!(value, 0xAB);

    // Verifies every queued transaction was consumed; this is what catches a
    // driver that talks to the bus differently than the test claims.
    i2c.done();
}
