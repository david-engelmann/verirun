//! Parser and construction fault table (INV-CAN-6, INV-CAN-7).

use verirun_core::canonical::to_vec;
use verirun_core::error::{CanonicalErrorKind, ParseErrorKind};
use verirun_core::parse::{parse, MAX_DEPTH};
use verirun_core::value::Value;

#[test]
fn fault_parser_rejection_table() {
    let cases: &[(&[u8], ParseErrorKind)] = &[
        (b"", ParseErrorKind::EmptyInput),
        (b"{", ParseErrorKind::UnexpectedEnd),
        (b"{\"a\":1,\"a\":2}", ParseErrorKind::DuplicateKey),
        (b"{\"a\":1} x", ParseErrorKind::TrailingBytes),
        (b"\"\\uD800\"", ParseErrorKind::LoneSurrogate),
        (b"\"\\uDC00\"", ParseErrorKind::LoneSurrogate),
        (b"\"\\uD800\\u0041\"", ParseErrorKind::LoneSurrogate),
        (b"\"a\x01b\"", ParseErrorKind::ControlCharacterInString),
        (b"01", ParseErrorKind::InvalidNumber),
        (b"+1", ParseErrorKind::UnexpectedByte),
        (b"1.", ParseErrorKind::InvalidNumber),
        (b"1e999", ParseErrorKind::NumberOutOfRange),
        (b"9007199254740992", ParseErrorKind::IntegerOutOfSafeRange),
        (b"\"\\x\"", ParseErrorKind::InvalidEscape),
        (b"\"\\u12G4\"", ParseErrorKind::InvalidUnicodeEscape),
        (b"nul", ParseErrorKind::UnexpectedEnd),
        (b"\"abc", ParseErrorKind::UnexpectedEnd),
        (b"[1,]", ParseErrorKind::UnexpectedByte),
        (b"{\"a\" 1}", ParseErrorKind::UnexpectedByte),
    ];
    for (input, kind) in cases {
        let err = parse(input).expect_err("must reject");
        assert_eq!(
            err.kind(),
            *kind,
            "input {:?}",
            String::from_utf8_lossy(input)
        );
    }

    // Invalid UTF-8 inside a string is rejected at its byte offset.
    let err = parse(b"\"\xFF\"").expect_err("invalid UTF-8");
    assert_eq!(err.kind(), ParseErrorKind::InvalidUtf8);
    assert_eq!(err.offset(), 1);

    // Depth exactly at the limit parses; one deeper is rejected.
    let mut at_limit = vec![b'['; MAX_DEPTH];
    at_limit.extend(vec![b']'; MAX_DEPTH]);
    parse(&at_limit).expect("depth at the limit parses");
    let mut past_limit = vec![b'['; MAX_DEPTH + 1];
    past_limit.extend(vec![b']'; MAX_DEPTH + 1]);
    let err = parse(&past_limit).expect_err("depth past the limit");
    assert_eq!(err.kind(), ParseErrorKind::DepthLimit);
}

#[test]
fn fault_duplicate_key_rejected_at_construction() {
    let pairs = vec![
        ("a".to_string(), Value::Number(1.0)),
        ("a".to_string(), Value::Number(2.0)),
    ];
    let err = Value::object(pairs).expect_err("duplicate keys rejected");
    assert_eq!(err.kind(), CanonicalErrorKind::DuplicateKey);

    // The encoder is the second gate: an object built directly through
    // the enum variant with duplicate keys is still rejected, never
    // hashed.
    let direct = Value::Object(vec![
        ("a".to_string(), Value::Number(1.0)),
        ("a".to_string(), Value::Number(2.0)),
    ]);
    let err = to_vec(&direct).expect_err("encoder rejects duplicates");
    assert_eq!(err.kind(), CanonicalErrorKind::DuplicateKey);
}
