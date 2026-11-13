//! Fixed canonicalization vectors: RFC 8785 Appendix B cases that fall
//! inside the verirun number domain, the number domain boundaries, the
//! UTF-16 discriminating pair, and the string escaping table.

use verirun_core::canonical::{format_number, to_vec};
use verirun_core::error::{CanonicalErrorKind, ParseErrorKind};
use verirun_core::parse::parse;
use verirun_core::value::Value;

fn encode_parsed(json: &str) -> String {
    let value = parse(json.as_bytes()).expect("test input must parse");
    let bytes = to_vec(&value).expect("test value must encode");
    String::from_utf8(bytes).expect("canonical output is UTF-8")
}

#[test]
fn vec_utf16_discriminating_pair() {
    // UTF-16 order: "a" (0061), then the supplementary-plane key whose
    // first code unit is a high surrogate (D83D), then U+E000. Code
    // point order would place U+E000 before U+1F600.
    let value = Value::object(vec![
        ("\u{E000}".to_string(), Value::Number(1.0)),
        ("\u{1F600}".to_string(), Value::Number(2.0)),
        ("a".to_string(), Value::Number(3.0)),
    ])
    .expect("unique keys");
    let bytes = to_vec(&value).expect("encodes");
    let expected = format!(
        "{{\"a\":3,\"{}\":2,\"{}\":1}}",
        "\u{1F600}", "\u{E000}"
    );
    assert_eq!(String::from_utf8(bytes).expect("UTF-8"), expected);
}

#[test]
fn vec_rfc8785_appendix_b_in_domain() {
    let cases: &[(&str, &str)] = &[
        (r#"{"b":1,"a":2}"#, r#"{"a":2,"b":1}"#),
        (
            r#"{"z":{"b":1,"a":2},"a":1}"#,
            r#"{"a":1,"z":{"a":2,"b":1}}"#,
        ),
        (r#"{"numbers":[1,2,3]}"#, r#"{"numbers":[1,2,3]}"#),
        (
            r#"{"literals":[null,true,false]}"#,
            r#"{"literals":[null,true,false]}"#,
        ),
        ("333333333.33333329", "333333333.3333333"),
        ("1.0", "1"),
        ("-0", "0"),
        ("0.000001", "0.000001"),
        ("0.0000001", "1e-7"),
        ("2e-3", "0.002"),
        ("4.50", "4.5"),
        ("1.5", "1.5"),
        ("-1.5", "-1.5"),
        (r#""A""#, r#""A""#),
        ("[]", "[]"),
        ("{}", "{}"),
        (r#""""#, r#""""#),
    ];
    for (input, expected) in cases {
        assert_eq!(&encode_parsed(input), expected, "input {input}");
    }
}

#[test]
fn vec_number_domain_boundaries() {
    let accepted: &[(f64, &str)] = &[
        (0.0, "0"),
        (-0.0, "0"),
        (1.0, "1"),
        (100.0, "100"),
        (2.5, "2.5"),
        (123.456, "123.456"),
        (0.1, "0.1"),
        (0.000_001, "0.000001"),
        (1e15, "1000000000000000"),
        (9_007_199_254_740_991.0, "9007199254740991"),
        (-9_007_199_254_740_991.0, "-9007199254740991"),
        (4_503_599_627_370_495.5, "4503599627370495.5"),
        (5e-324, "5e-324"),
    ];
    for (value, expected) in accepted {
        assert_eq!(
            &format_number(*value).expect("in domain"),
            expected,
            "value {value}"
        );
    }

    let rejected: &[(f64, CanonicalErrorKind)] = &[
        (9_007_199_254_740_992.0, CanonicalErrorKind::IntegerOutOfSafeRange),
        (1e20, CanonicalErrorKind::IntegerOutOfSafeRange),
        (1e21, CanonicalErrorKind::IntegerOutOfSafeRange),
        (1.5e21, CanonicalErrorKind::IntegerOutOfSafeRange),
        (1e30, CanonicalErrorKind::IntegerOutOfSafeRange),
        (f64::NAN, CanonicalErrorKind::NonFiniteNumber),
        (f64::INFINITY, CanonicalErrorKind::NonFiniteNumber),
        (f64::NEG_INFINITY, CanonicalErrorKind::NonFiniteNumber),
    ];
    for (value, kind) in rejected {
        let err = format_number(*value).expect_err("out of domain");
        assert_eq!(err.kind(), *kind, "value {value}");
    }

    let err = parse(b"9007199254740992").expect_err("integral above bound");
    assert_eq!(err.kind(), ParseErrorKind::IntegerOutOfSafeRange);
    let err = parse(b"1e999").expect_err("overflow literal");
    assert_eq!(err.kind(), ParseErrorKind::NumberOutOfRange);
    let err = parse(b"1.5e21").expect_err("integral value above bound");
    assert_eq!(err.kind(), ParseErrorKind::IntegerOutOfSafeRange);
}

#[test]
fn vec_escaping_table() {
    let mut raw = String::new();
    for code in 0x00_u32..=0x1F {
        raw.push(char::from_u32(code).expect("control range is valid"));
    }
    raw.push('"');
    raw.push('\\');
    raw.push('/');
    raw.push('\u{7F}');
    raw.push_str("A\u{E9}\u{1F600}");

    // The expected form is written out from the RFC 8785 section
    // 3.2.2.2 rules directly, independently of the encoder.
    let mut expected = String::from("\"");
    for ch in raw.chars() {
        match ch {
            '"' => expected.push_str("\\\""),
            '\\' => expected.push_str("\\\\"),
            '\u{08}' => expected.push_str("\\b"),
            '\u{09}' => expected.push_str("\\t"),
            '\u{0A}' => expected.push_str("\\n"),
            '\u{0C}' => expected.push_str("\\f"),
            '\u{0D}' => expected.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                expected.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => expected.push(c),
        }
    }
    expected.push('"');

    let bytes = to_vec(&Value::String(raw)).expect("encodes");
    assert_eq!(String::from_utf8(bytes).expect("UTF-8"), expected);
}
