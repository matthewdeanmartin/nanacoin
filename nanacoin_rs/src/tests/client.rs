use super::*;
#[test]
fn quantities_are_exact_bounded_decimal_values() {
    for (text, value) in [
        ("0.001", 1),
        ("1", 1000),
        ("1.2", 1200),
        ("1.23", 1230),
        ("1.234", 1234),
        ("1000000", 1_000_000_000),
    ] {
        assert_eq!(quantity_milli(text), Ok(value));
    }
    for text in [
        "",
        ".1",
        "0",
        "0.000",
        "-1",
        "+1",
        "1e3",
        "1.0001",
        "1..2",
        "1. 2",
        "4294967296",
        "4294967295",
        "1000000.001",
        "١",
        "1\0",
        " 1",
    ] {
        assert_eq!(quantity_milli(text), Err(Error::InvalidInput), "{text:?}");
    }
    for text in [
        "user-0",
        "user-256",
        "user--1",
        "user-99999999999999999999",
        "account-2",
        "user-",
    ] {
        assert_eq!(member_id(text, "user-"), Err(Error::InvalidInput));
    }
    assert_eq!(member_id("user-255", "user-"), Ok(MemberId(255)));
    assert_eq!(number("tx-18446744073709551615", "tx-"), Ok(u64::MAX));
    assert_eq!(
        number("tx-18446744073709551616", "tx-"),
        Err(Error::InvalidInput)
    );
}
