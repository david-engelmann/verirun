//! The verirun executable: a thin shell over the library in
//! `lib.rs` (docs/ARCHITECTURE.md section 2.3).
//!
//! Milestone 2 lands the run pipeline and manifest verification
//! in the library. The command surface of section 12.1 (run,
//! verify, diff, gc, with seed reporting and `--json`) is wired
//! in Milestone 3. Until then the executable says so and exits
//! with code 3, invalid input, never 2: code 2 is reserved for
//! integrity failures, and a usage state must never report as
//! tampering.

fn main() {
    eprintln!(
        "verirun: the command surface lands in Milestone 3; \
         the run pipeline and manifest verification are in the verirun-cli library"
    );
    std::process::exit(3);
}
