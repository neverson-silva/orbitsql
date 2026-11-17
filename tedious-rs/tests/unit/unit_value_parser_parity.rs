use tedious_rs::metadata_parser::read_metadata;
use tedious_rs::value_codec::Value;
use tedious_rs::value_parser::read_value_with_options;

fn metadata(id: u8, extra: &[u8]) -> tedious_rs::metadata_parser::Metadata {
    let mut bytes = vec![0, 0, 0, 0, 0, 0, id];
    bytes.extend_from_slice(extra);
    read_metadata(&bytes, 0, "7_4").unwrap().value
}

#[test]
fn money_matches_node_wire_order_and_preserves_scaled_integer() {
    let money = metadata(0x3C, &[]);
    let cases = [
        (vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], 0_i128),
        (
            vec![0x20, 0x4B, 0xBC, 0x00, 0x00, 0x00, 0x00, 0x00],
            12_340_000_i128,
        ),
        (
            vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F],
            9_223_372_036_854_775_807_i128,
        ),
        (
            vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80],
            -9_223_372_036_854_775_808_i128,
        ),
    ];

    for (wire, unscaled) in cases {
        let parsed = read_value_with_options(&wire, 0, &money, true).unwrap();
        assert_eq!(parsed.offset, 8);
        assert_eq!(parsed.value, Value::Decimal { unscaled, scale: 4 });
    }
}

#[test]
fn smallmoney_matches_node_wire_order_and_preserves_scaled_integer() {
    let smallmoney = metadata(0x7A, &[]);
    let cases = [
        (vec![0, 0, 0, 0], 0_i128),
        (vec![0x20, 0x4E, 0x00, 0x00], 20_000_i128),
        (vec![0xFF, 0xFF, 0xFF, 0x7F], 2_147_483_647_i128),
        (vec![0x00, 0x00, 0x00, 0x80], -2_147_483_648_i128),
    ];

    for (wire, unscaled) in cases {
        let parsed = read_value_with_options(&wire, 0, &smallmoney, true).unwrap();
        assert_eq!(parsed.offset, 4);
        assert_eq!(parsed.value, Value::Decimal { unscaled, scale: 4 });
    }
}

#[test]
fn money_n_null_and_widths_match_node() {
    let money_n = metadata(0x6E, &[8]);
    assert_eq!(
        read_value_with_options(&[0], 0, &money_n, true)
            .unwrap()
            .value,
        Value::Null
    );
    assert_eq!(
        read_value_with_options(&[4, 0x20, 0x4E, 0, 0], 0, &money_n, true)
            .unwrap()
            .value,
        Value::Decimal {
            unscaled: 20_000,
            scale: 4
        }
    );
    assert_eq!(
        read_value_with_options(&[8, 0x20, 0x4B, 0xBC, 0, 0, 0, 0, 0, 0], 0, &money_n, true,)
            .unwrap()
            .value,
        Value::Decimal {
            unscaled: 12_340_000,
            scale: 4
        }
    );
}

#[test]
fn temporal_values_preserve_node_millisecond_and_submillisecond_observations() {
    let time = metadata(0x29, &[7]);
    let parsed =
        read_value_with_options(&[5, 0x80, 0xC0, 0x51, 0xAB, 0x08], 0, &time, true).unwrap();
    assert_eq!(
        parsed.value,
        Value::Time {
            hour: 1,
            minute: 2,
            second: 3,
            nanosecond: 400_000_000
        }
    );

    let local =
        read_value_with_options(&[5, 0x80, 0xC0, 0x51, 0xAB, 0x08], 0, &time, false).unwrap();
    assert_eq!(
        local.value,
        Value::LocalTime {
            hour: 1,
            minute: 2,
            second: 3,
            nanosecond: 400_000_000
        }
    );
}

#[test]
fn datetimeoffset_preserves_wire_offset_while_matching_node_instant() {
    let datetimeoffset = metadata(0x2B, &[7]);
    let mut wire = vec![10, 0x80, 0xC0, 0x51, 0xAB, 0x08];
    wire.extend_from_slice(&719_162_u32.to_le_bytes()[..3]);
    wire.extend_from_slice(&(-330_i16).to_le_bytes());
    let parsed = read_value_with_options(&wire, 0, &datetimeoffset, false).unwrap();
    assert_eq!(
        parsed.value,
        Value::DateTimeOffset {
            date: Box::new(Value::Date {
                year: 1970,
                month: 1,
                day: 1
            }),
            time: Box::new(Value::Time {
                hour: 1,
                minute: 2,
                second: 3,
                nanosecond: 400_000_000
            }),
            offset_minutes: -330
        }
    );
    assert_eq!(parsed.offset, wire.len());
}
