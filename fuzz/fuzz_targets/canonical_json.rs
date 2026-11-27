#![no_main]

use libfuzzer_sys::fuzz_target;
use verirun_core::{canonical, parse};

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = parse::parse(data) {
        let encoded = canonical::to_vec(&value)
            .expect("every value accepted by parse::parse must be in the canonical domain");
        let reparsed =
            parse::parse(&encoded).expect("canonical RFC 8785 bytes must always re-parse");
        assert_eq!(reparsed, value, "canonical round-trip must preserve Value");
        let reencoded =
            canonical::to_vec(&reparsed).expect("re-parsed canonical value must re-encode cleanly");
        assert_eq!(
            reencoded, encoded,
            "canonical encoding must be a byte-identical fixed point"
        );
    }
});
