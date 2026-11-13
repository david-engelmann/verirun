//! Property tests for the canonical encoder and parser.

use proptest::prelude::*;
use verirun_core::canonical::{format_number, to_vec};
use verirun_core::parse::parse;
use verirun_core::value::Value;

/// In-domain numbers: safe integers, moderate fractions built from
/// decimal parts, and small magnitudes from scientific construction.
fn arb_number() -> impl Strategy<Value = f64> {
    prop_oneof![
        (-9_007_199_254_740_991_i64..=9_007_199_254_740_991_i64)
            .prop_map(|i| i as f64),
        (0_i64..1_000_000, 0_u32..1_000_000, 1_u32..7).prop_map(|(whole, frac, digits)| {
            let scale = 10_f64.powi(i32::try_from(digits).expect("small"));
            whole as f64 + frac as f64 / scale / 1000.0
        }),
        (1_i64..999, 7_u32..30).prop_filter_map("finite small", |(mantissa, exp)| {
            format!("{mantissa}e-{exp}").parse::<f64>().ok()
        }),
        (0_u32..10_000).prop_map(|i| f64::from(i) / 3.0),
    ]
}

fn arb_string() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z0-9 _-]{0,12}",
        any::<char>().prop_map(|c| c.to_string()),
        "\\PC{0,16}",
    ]
}

fn arb_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        arb_number().prop_map(|n| Value::Number(n)),
        arb_string().prop_map(Value::String),
    ];
    leaf.prop_recursive(4, 48, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Value::Array),
            prop::collection::vec((arb_string(), inner), 0..5).prop_filter_map(
                "unique keys",
                |pairs| Value::object(pairs).ok(),
            ),
        ]
    })
}

proptest! {
    #[test]
    fn prop_parse_encode_roundtrip(value in arb_value()) {
        let bytes = to_vec(&value).expect("generated values are in domain");
        let back = parse(&bytes).expect("canonical bytes parse");
        prop_assert_eq!(&back, &value);
        let bytes_again = to_vec(&back).expect("re-encodes");
        prop_assert_eq!(bytes_again, bytes);
    }

    #[test]
    fn prop_key_order_matches_utf16_reference(
        keys in prop::collection::vec(arb_string(), 1..10),
        values in prop::collection::vec(arb_number(), 1..10),
    ) {
        let mut seen = std::collections::BTreeSet::new();
        let mut pairs: Vec<(String, Value)> = Vec::new();
        for (index, key) in keys.iter().enumerate() {
            if seen.insert(key.clone()) {
                let number = values[index % values.len()];
                pairs.push((key.clone(), Value::Number(number)));
            }
        }
        prop_assume!(!pairs.is_empty());
        let object = Value::object(pairs).expect("unique keys");
        let bytes = to_vec(&object).expect("encodes");
        let parsed = parse(&bytes).expect("canonical bytes parse");
        let Value::Object(round_tripped) = parsed else {
            panic!("expected an object");
        };
        let actual_order: Vec<&str> =
            round_tripped.iter().map(|(key, _)| key.as_str()).collect();
        let mut reference: Vec<Vec<u16>> =
            seen.iter().map(|key| key.encode_utf16().collect()).collect();
        reference.sort();
        let reference_strings: Vec<String> = reference
            .iter()
            .map(|units| String::from_utf16(units).expect("valid UTF-16"))
            .collect();
        let reference_refs: Vec<&str> =
            reference_strings.iter().map(String::as_str).collect();
        prop_assert_eq!(actual_order, reference_refs);
    }

    #[test]
    fn prop_number_layout_matches_oracle(value in arb_number()) {
        let expected = oracle_format(value);
        let actual = format_number(value).expect("in domain");
        prop_assert_eq!(actual, expected);
    }

    #[test]
    fn prop_no_unlisted_escapes(text in any::<String>()) {
        let bytes = to_vec(&Value::String(text)).expect("encodes");
        prop_assert_eq!(bytes.first(), Some(&b'"'));
        prop_assert_eq!(bytes.last(), Some(&b'"'));
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'\\' {
                let next = bytes[index + 1];
                prop_assert!(
                    matches!(next, b'"' | b'\\' | b'b' | b't' | b'n' | b'f' | b'r' | b'u'),
                    "unlisted escape at {index}"
                );
                if next == b'u' {
                    let hex = &bytes[index + 2..index + 6];
                    prop_assert!(
                        hex.iter().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
                        "escape hex must be lowercase"
                    );
                    let code = u32::from_str_radix(
                        std::str::from_utf8(hex).expect("hex is ASCII"),
                        16,
                    )
                    .expect("four hex digits");
                    prop_assert!(code < 0x20, "only control characters use unicode escapes");
                    index += 6;
                } else {
                    index += 2;
                }
            } else {
                index += 1;
            }
        }
    }

    #[test]
    fn prop_parser_never_panics_on_mutation(bytes in prop::collection::vec(any::<u8>(), 0..256)) {
        // Any result is acceptable. A panic fails the test.
        let _ = parse(&bytes);
    }
}

/// An independent ECMA-262 `Number::toString` layout, written for this
/// test from the specification text over the digit source of the
/// standard library scientific formatter. The implementation under
/// test extracts digits from the plain decimal formatter instead, so
/// digit-extraction bugs cannot hide in both.
fn oracle_format(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent_text) = scientific
        .split_once('e')
        .expect("scientific form has an exponent");
    let exponent: i32 = exponent_text.parse().expect("exponent is an integer");
    let mut digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    let k = i32::try_from(digits.len()).expect("digit count fits");
    // value = 0.digits * 10^n, with the scientific form giving
    // value = d.igits * 10^exponent, so n = exponent + 1.
    let n = exponent + 1;
    let body = if k <= n && n <= 21 {
        let mut out = digits.clone();
        for _ in 0..(n - k) {
            out.push('0');
        }
        out
    } else if 0 < n && n <= 21 && n < k {
        let point = usize::try_from(n).expect("positive");
        format!("{}.{}", &digits[..point], &digits[point..])
    } else if -6 < n && n <= 0 {
        let zeros = usize::try_from(-n).expect("non-negative");
        format!("0.{}{}", "0".repeat(zeros), digits)
    } else if k == 1 {
        format!("{digits}e{}{}", if n - 1 >= 0 { "+" } else { "-" }, (n - 1).abs())
    } else {
        format!(
            "{}.{}e{}{}",
            &digits[..1],
            &digits[1..],
            if n - 1 >= 0 { "+" } else { "-" },
            (n - 1).abs()
        )
    };
    format!("{sign}{body}")
}

#[test]
fn prop_parser_never_panics_on_truncation() {
    let docs: &[&[u8]] = &[
        b"{\"a\":[1,2.5,\"x\\u0041\",{\"b\":null}],\"c\":true}",
        b"[333333333.33333329,{\"deep\":[[[[1]]]]}]",
        b"\"a string with \\n escapes and \\uD83D\\uDE00 pairs\"",
        b"{\"empty\":{},\"list\":[],\"n\":-0.5e2}",
    ];
    for doc in docs {
        parse(doc).expect("seed documents are valid");
        for cut in 0..doc.len() {
            // Any result is acceptable. A panic fails the test.
            let _ = parse(&doc[..cut]);
        }
    }
}
