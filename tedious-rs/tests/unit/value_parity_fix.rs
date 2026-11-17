use tedious_rs::value_codec::{Value, encode_smalldatetime};

fn temporal_value(hour: u8, minute: u8, second: u8, nanosecond: u32) -> Value {
    Value::DateTime {
        date: Box::new(Value::Date {
            year: 1900,
            month: 1,
            day: 1,
        }),
        time: Box::new(Value::Time {
            hour,
            minute,
            second,
            nanosecond,
        }),
    }
}

#[test]
fn smalldatetime_uses_node_hour_and_minute_without_second_rounding() {
    let encoded = encode_smalldatetime(&temporal_value(12, 34, 59, 999_999_999)).unwrap();
    assert_eq!(encoded.type_info, vec![0x6F, 4]);
    assert_eq!(encoded.length, vec![4]);
    assert_eq!(encoded.data, vec![0, 0, 0xF2, 2]);
}

#[test]
fn smalldatetime_matches_node_supported_calendar_range() {
    let valid = Value::DateTime {
        date: Box::new(Value::Date {
            year: 2079,
            month: 6,
            day: 6,
        }),
        time: Box::new(Value::Time {
            hour: 23,
            minute: 59,
            second: 59,
            nanosecond: 999_999_999,
        }),
    };
    assert!(encode_smalldatetime(&valid).is_ok());
    let invalid = Value::DateTime {
        date: Box::new(Value::Date {
            year: 2079,
            month: 6,
            day: 7,
        }),
        time: Box::new(Value::Time {
            hour: 0,
            minute: 0,
            second: 0,
            nanosecond: 0,
        }),
    };
    assert!(encode_smalldatetime(&invalid).is_err());
}
