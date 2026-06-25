//! File-driven snapshot tests: one case per file in `fixtures/`.
//!
//! This is the `insta` analog of cockroachdb/datadriven. Each fixture holds a
//! single tick count; the committed `.snap` files capture the exact heartbeat
//! line. Run `cargo insta review` to accept intended changes. In CI (`CI=1`)
//! insta runs in "no update" mode, so a stale snapshot fails the build.

use insta::{assert_snapshot, glob, with_settings};
use std::fs;

#[test]
fn heartbeat_lines() {
    glob!("fixtures/*.txt", |path| {
        let raw = fs::read_to_string(path).expect("fixture readable");
        let count: u32 = raw.trim().parse().expect("fixture is a u32 tick count");

        // Name each snapshot after its fixture file so the .snap is easy to map
        // back to its input.
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        // Snapshot against the primary board; per-board tagging is covered by the
        // inline `heartbeat_tags_the_board` test in the crate.
        with_settings!({ snapshot_suffix => stem }, {
            assert_snapshot!(aq_core::heartbeat_line("aq-indoor", count).as_str());
        });
    });
}
