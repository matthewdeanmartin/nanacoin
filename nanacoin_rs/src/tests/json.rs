use super::*;
#[derive(Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Text {
    text: heapless::String<128>,
}

#[test]
fn escaped_strings_match_the_reference_decoder() {
    for content in [
        "hello",
        "🎁",
        "𐀀",
        "\u{10ffff}",
        "é中",
        "\\uD800\\uDC00",
        "\"\n\r\t\0",
        "\\\\\\u1234",
    ] {
        let bytes = serde_json::to_vec(&serde_json::json!({"text":content})).unwrap();
        let actual: Text = decode(&bytes).unwrap();
        let reference: Text = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(actual, reference);
    }
    for high in [0xd800u32, 0xd801, 0xdbfe, 0xdbff] {
        for low in [0xdc00u32, 0xdc01, 0xdffe, 0xdfff] {
            let bytes = format!("{{\"text\":\"\\u{high:04X}\\u{low:04X}\"}}");
            assert_eq!(
                decode::<Text>(bytes.as_bytes()).unwrap(),
                serde_json::from_str::<Text>(&bytes).unwrap()
            );
        }
    }
}

#[test]
fn malformed_escapes_utf8_duplicate_fields_and_trailing_values_are_errors() {
    for input in [
        r#"{"text":"\uD800"}"#,
        r#"{"text":"\uDC00"}"#,
        r#"{"text":"\uD800\u1234"}"#,
        r#"{"text":"\uD800\uZZZZ"}"#,
        r#"{"text":"\u000g"}"#,
        r#"{"text":"\x41"}"#,
        r#"{"text":"one","text":"two"}"#,
        r#"{"text":"one","admin":true}"#,
        r#"{"text":"one"} null"#,
    ] {
        assert!(decode::<Text>(input.as_bytes()).is_err(), "{input}");
    }
    assert!(decode::<Text>(b"{\"text\":\"\xff\"}").is_err());
    let valid = br#"{"text":"\uD83C\uDF81"}"#;
    for end in 0..valid.len() {
        assert!(decode::<Text>(&valid[..end]).is_err(), "truncation {end}");
    }
    assert!(decode::<Text>(&vec![b' '; 1025]).is_err());
    assert_eq!(
        decode::<Text>(b"{\"text\":\"ok\"} \r\n\t").unwrap().text,
        "ok"
    );
}
