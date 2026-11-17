use tedious_rs::collation::Collation;
use tedious_rs::value_codec::{
    Value, encode_bigint, encode_binary, encode_bit, encode_char, encode_date, encode_datetime,
    encode_datetime2, encode_datetimeoffset, encode_decimal, encode_float, encode_image,
    encode_int, encode_money, encode_nchar, encode_ntext, encode_ntext_with_collation,
    encode_nvarchar, encode_real, encode_smalldatetime, encode_smallint, encode_smallmoney,
    encode_text, encode_time, encode_tinyint, encode_tvp, encode_uniqueidentifier,
    encode_varbinary, encode_varchar, encode_xml,
};

#[test]
fn encodes_integer_type_info_lengths_and_little_endian_data() {
    assert_eq!(
        encode_tinyint(&Value::Integer(255)).unwrap().data,
        vec![255]
    );
    assert_eq!(
        encode_smallint(&Value::Integer(-2)).unwrap().data,
        vec![254, 255]
    );
    assert_eq!(
        encode_int(&Value::Integer(-2)).unwrap().data,
        vec![254, 255, 255, 255]
    );
    assert_eq!(
        encode_bigint(&Value::BigInteger("-2".to_owned()))
            .unwrap()
            .data,
        vec![254, 255, 255, 255, 255, 255, 255, 255]
    );
    assert_eq!(encode_int(&Value::Null).unwrap().length, vec![0]);
}

#[test]
fn validates_integer_bounds_and_boolean_wire_values() {
    assert!(encode_tinyint(&Value::Integer(256)).is_err());
    assert!(encode_smallint(&Value::Integer(-32769)).is_err());
    assert_eq!(encode_bit(&Value::Boolean(true)).unwrap().data, vec![1]);
    assert_eq!(encode_bit(&Value::Boolean(false)).unwrap().data, vec![0]);
    assert_eq!(encode_bit(&Value::Integer(1)).unwrap().data, vec![1]);
}

#[test]
fn accepts_node_numeric_coercions_for_scalar_codecs() {
    assert_eq!(
        encode_int(&Value::Text("12.9".to_owned())).unwrap().data,
        12_i32.to_le_bytes()
    );
    assert_eq!(
        encode_smallint(&Value::Float(7.8)).unwrap().data,
        7_i16.to_le_bytes()
    );
    assert_eq!(
        encode_bigint(&Value::Text("9223372036854775807".to_owned()))
            .unwrap()
            .data,
        i64::MAX.to_le_bytes()
    );
    assert_eq!(
        encode_real(&Value::Text("1.25suffix".to_owned()))
            .unwrap()
            .data,
        1.25_f32.to_le_bytes()
    );
    assert_eq!(
        encode_float(&Value::Text(".5suffix".to_owned()))
            .unwrap()
            .data,
        0.5_f64.to_le_bytes()
    );
    assert_eq!(
        encode_float(&Value::Integer(3)).unwrap().data,
        3_f64.to_le_bytes()
    );
}

#[test]
fn integer_strings_follow_node_number_base_and_empty_string_coercions() {
    assert_eq!(
        encode_int(&Value::Text("".to_owned())).unwrap().data,
        0_i32.to_le_bytes()
    );
    assert_eq!(
        encode_int(&Value::Text("0x10".to_owned())).unwrap().data,
        16_i32.to_le_bytes()
    );
    assert_eq!(
        encode_smallint(&Value::Text("0b101".to_owned()))
            .unwrap()
            .data,
        5_i16.to_le_bytes()
    );
    assert_eq!(
        encode_tinyint(&Value::Text("0o10".to_owned()))
            .unwrap()
            .data,
        vec![8]
    );
    assert!(encode_int(&Value::Text("15tail".to_owned())).is_err());
}

#[test]
fn bigint_strings_follow_node_bigint_radix_coercions() {
    assert_eq!(
        encode_bigint(&Value::Text("0x10".to_owned())).unwrap().data,
        16_i64.to_le_bytes()
    );
    assert_eq!(
        encode_bigint(&Value::Text("0b101".to_owned()))
            .unwrap()
            .data,
        5_i64.to_le_bytes()
    );
    assert_eq!(
        encode_bigint(&Value::Text("0o10".to_owned())).unwrap().data,
        8_i64.to_le_bytes()
    );
    assert_eq!(
        encode_bigint(&Value::Text("".to_owned())).unwrap().data,
        0_i64.to_le_bytes()
    );
    assert!(encode_bigint(&Value::Text("1.0".to_owned())).is_err());
}

#[test]
fn bigint_strings_match_node_sign_rules_for_decimal_and_radix_literals() {
    for value in ["-1", "+1", "-0", "+0"] {
        assert!(encode_bigint(&Value::BigInteger(value.to_owned())).is_ok());
    }
    for value in ["-0x1", "+0x1", "-0b1", "+0b1", "-0o1", "+0o1"] {
        assert!(encode_bigint(&Value::BigInteger(value.to_owned())).is_err());
    }
}

#[test]
fn encodes_ieee_values_with_matching_tds_type_info() {
    let real = encode_real(&Value::Real(1.5)).unwrap();
    let float = encode_float(&Value::Float(1.5)).unwrap();
    assert_eq!(real.type_info, vec![0x6D, 4]);
    assert_eq!(float.type_info, vec![0x6D, 8]);
    assert_eq!(real.data, 1.5f32.to_le_bytes());
    assert_eq!(float.data, 1.5f64.to_le_bytes());
}

#[test]
fn encodes_oem_collations_supported_by_iconv_lite() {
    let cp437 = Collation::new(0x0409, 0, 0, 30);
    let cp850 = Collation::new(0x0409, 0, 0, 40);
    let cp949 = Collation::new(0x0412, 0, 0, 0);
    assert_eq!(
        encode_varchar(&Value::Text("é".to_owned()), 1, &cp437)
            .unwrap()
            .data,
        vec![0x82]
    );
    assert_eq!(
        encode_varchar(&Value::Text("é".to_owned()), 1, &cp850)
            .unwrap()
            .data,
        vec![0x82]
    );
    assert_eq!(
        encode_varchar(&Value::Text("한글".to_owned()), 4, &cp949)
            .unwrap()
            .data,
        vec![0xC7, 0xD1, 0xB1, 0xDB]
    );
}

#[test]
fn encodes_binary_null_and_truncates_to_binary_maximum() {
    let value = Value::Binary((0..9000).map(|v| (v % 256) as u8).collect());
    let encoded = encode_binary(&value, 9000).unwrap();
    assert_eq!(encoded.type_info, vec![0xAD, 0x28, 0x23]);
    assert_eq!(encoded.data.len(), 8000);
    assert_eq!(
        encode_binary(&Value::Null, 4).unwrap().length,
        vec![0xFF, 0xFF]
    );
}

#[test]
fn encodes_guid_in_sql_server_byte_order() {
    let encoded = encode_uniqueidentifier(&Value::Guid(
        "00112233-4455-6677-8899-aabbccddeeff".to_owned(),
    ))
    .unwrap();
    assert_eq!(encoded.type_info, vec![0x24, 16]);
    assert_eq!(
        encoded.data,
        vec![
            0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff
        ]
    );
    assert!(encode_uniqueidentifier(&Value::Guid("invalid".to_owned())).is_err());
}

#[test]
fn encodes_tvp_with_declared_schema_columns_and_rows() {
    let value = Value::Tvp(Box::new(tedious_rs::payloads::TvpPayload {
        database: String::new(),
        schema: "dbo".to_owned(),
        type_name: "item_type".to_owned(),
        columns: vec![tedious_rs::payloads::TvpColumnDefinition {
            name: "id".to_owned(),
            type_info: vec![0x38],
        }],
        rows: vec![tedious_rs::payloads::TvpRow {
            values: vec![encode_int(&Value::Integer(7)).unwrap()],
        }],
    }));
    let encoded = encode_tvp(&value).unwrap();
    assert_eq!(encoded.type_info[0], 0xF3);
    assert_eq!(encoded.length, vec![1, 0]);
    assert!(encoded.data.windows(2).any(|pair| pair == [0x01, 0x04]));
    let null = encode_tvp(&Value::Null).unwrap();
    assert_eq!(null.type_info, vec![0xF3, 0, 0, 0]);
    assert_eq!(null.length, vec![0xFF, 0xFF]);
    assert_eq!(null.data, vec![0, 0]);
}

#[test]
fn encodes_date_time_and_datetime2_using_tds_epoch_and_scale_widths() {
    let date = encode_date(&Value::Date {
        year: 1,
        month: 1,
        day: 1,
    })
    .unwrap();
    assert_eq!(date.type_info, vec![0x28]);
    assert_eq!(date.length, vec![3]);
    assert_eq!(date.data, vec![0, 0, 0]);
    let time = encode_time(
        &Value::Time {
            hour: 1,
            minute: 2,
            second: 3,
            nanosecond: 400_000_000,
        },
        3,
    )
    .unwrap();
    assert_eq!(time.type_info, vec![0x29, 3]);
    assert_eq!(time.length, vec![4]);
    assert_eq!(time.data, 3_723_400_i64.to_le_bytes()[..4].to_vec());
    let datetime = encode_datetime2(
        &Value::DateTime {
            date: Box::new(Value::Date {
                year: 1,
                month: 1,
                day: 1,
            }),
            time: Box::new(Value::Time {
                hour: 0,
                minute: 0,
                second: 0,
                nanosecond: 0,
            }),
        },
        7,
    )
    .unwrap();
    assert_eq!(datetime.type_info, vec![0x2A, 7]);
    assert_eq!(datetime.length, vec![8]);
    assert_eq!(datetime.data, vec![0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn encodes_datetimeoffset_and_decimal_sign_magnitude() {
    let offset = encode_datetimeoffset(
        &Value::DateTimeOffset {
            date: Box::new(Value::Date {
                year: 1,
                month: 1,
                day: 1,
            }),
            time: Box::new(Value::Time {
                hour: 0,
                minute: 0,
                second: 0,
                nanosecond: 0,
            }),
            offset_minutes: -180,
        },
        0,
    )
    .unwrap();
    assert_eq!(offset.length, vec![8]);
    assert_eq!(&offset.data[3..5], &[0, 0]);
    assert_eq!(&offset.data[6..], &(-180i16).to_le_bytes());
    let decimal = encode_decimal(
        &Value::Decimal {
            unscaled: -12345,
            scale: 2,
        },
        9,
        2,
    )
    .unwrap();
    assert_eq!(decimal.type_info, vec![0x6A, 5, 9, 2]);
    assert_eq!(decimal.length, vec![5]);
    assert_eq!(decimal.data, vec![0, 0x39, 0x30, 0, 0]);
    assert!(
        encode_decimal(
            &Value::Decimal {
                unscaled: 1,
                scale: 3
            },
            9,
            2
        )
        .is_err()
    );
    assert_eq!(
        encode_decimal(&Value::Text("12.34tail".to_owned()), 9, 2)
            .unwrap()
            .data,
        vec![1, 0xD2, 0x04, 0, 0]
    );
}

#[test]
fn encodes_legacy_datetime_money_and_smalldatetime_formats() {
    let value = Value::DateTime {
        date: Box::new(Value::Date {
            year: 1900,
            month: 1,
            day: 1,
        }),
        time: Box::new(Value::Time {
            hour: 1,
            minute: 0,
            second: 0,
            nanosecond: 0,
        }),
    };
    let datetime = encode_datetime(&value).unwrap();
    assert_eq!(datetime.type_info, vec![0x6F, 8]);
    assert_eq!(datetime.data, vec![0, 0, 0, 0, 0xC0, 0x7A, 0x10, 0]);
    let small_datetime = encode_smalldatetime(&value).unwrap();
    assert_eq!(small_datetime.type_info, vec![0x6F, 4]);
    assert_eq!(small_datetime.data, vec![0, 0, 60, 0]);
    assert_eq!(
        encode_money(&Value::Decimal {
            unscaled: -123456,
            scale: 4
        })
        .unwrap()
        .data,
        (-123456i64).to_le_bytes()
    );
    assert_eq!(
        encode_smallmoney(&Value::Decimal {
            unscaled: 123456,
            scale: 4
        })
        .unwrap()
        .data,
        (123456i32).to_le_bytes()
    );
    assert_eq!(
        encode_money(&Value::Text("12.3456".to_owned()))
            .unwrap()
            .data,
        123456_i64.to_le_bytes()
    );
    assert_eq!(
        encode_smallmoney(&Value::Float(12.3456)).unwrap().data,
        123456_i32.to_le_bytes()
    );
}

#[test]
fn bigint_accepts_integral_numeric_values_and_rejects_fractions() {
    assert_eq!(
        encode_bigint(&Value::Float(2_147_483_647.0)).unwrap().data,
        2_147_483_647_i64.to_le_bytes()
    );
    assert_eq!(
        encode_bigint(&Value::Boolean(true)).unwrap().data,
        1_i64.to_le_bytes()
    );
    assert!(encode_bigint(&Value::Float(0.5)).is_err());
}

#[test]
fn integer_types_coerce_booleans_like_node_number_validation() {
    assert_eq!(encode_tinyint(&Value::Boolean(true)).unwrap().data, vec![1]);
    assert_eq!(
        encode_smallint(&Value::Boolean(false)).unwrap().data,
        0_i16.to_le_bytes()
    );
    assert_eq!(
        encode_int(&Value::Boolean(true)).unwrap().data,
        1_i32.to_le_bytes()
    );
}

#[test]
fn rejects_datetime_values_outside_the_node_legacy_range() {
    let before_minimum = Value::DateTime {
        date: Box::new(Value::Date {
            year: 1752,
            month: 12,
            day: 31,
        }),
        time: Box::new(Value::Time {
            hour: 0,
            minute: 0,
            second: 0,
            nanosecond: 0,
        }),
    };
    assert!(encode_datetime(&before_minimum).is_err());

    let maximum = Value::DateTime {
        date: Box::new(Value::Date {
            year: 9999,
            month: 12,
            day: 31,
        }),
        time: Box::new(Value::Time {
            hour: 23,
            minute: 59,
            second: 59,
            nanosecond: 999_000_000,
        }),
    };
    assert!(encode_datetime(&maximum).is_ok());
}

#[test]
fn encodes_unicode_collated_and_plp_string_binary_parameters() {
    let collation = Collation::new(0x409, 0x80000000, 0, 0);
    let varchar = encode_varchar(&Value::Text("Aß".to_owned()), 8000, &collation).unwrap();
    assert_eq!(varchar.type_info[0], 0xA7);
    assert_eq!(varchar.length, vec![2, 0]);
    assert_eq!(varchar.data, vec![b'A', 0xDF]);
    let nvarchar = encode_nvarchar(&Value::Text("Aß".to_owned()), 4000).unwrap();
    assert_eq!(nvarchar.type_info, vec![0xE7, 0x40, 0x1F, 0, 0, 0, 0, 0]);
    assert_eq!(nvarchar.data, vec![b'A', 0, 0xDF, 0]);
    let plp = encode_varbinary(&Value::Binary(vec![1, 2, 3]), 8001).unwrap();
    assert_eq!(
        plp.length,
        vec![0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(plp.data, vec![3, 0, 0, 0, 1, 2, 3, 0, 0, 0, 0]);
    assert_eq!(
        encode_nvarchar(&Value::Null, 4001).unwrap().length,
        vec![0xFF; 8]
    );
}

#[test]
fn varchar_max_uses_the_node_maximum_type_info_length() {
    let collation = Collation::new(0x409, 0, 0, 0);
    let encoded = encode_varchar(&Value::Text("x".to_owned()), 8001, &collation).unwrap();
    assert_eq!(&encoded.type_info[1..3], &[0xFF, 0xFF]);
    assert_eq!(
        encoded.length,
        vec![0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
}

#[test]
fn encodes_legacy_lob_lengths_and_xml() {
    let collation = Collation::new(0x409, 0, 0, 0);
    let text = encode_text(&Value::Text("Aß".to_owned()), -1, &collation).unwrap();
    assert_eq!(text.type_info[0], 0x23);
    assert_eq!(text.length, vec![2, 0, 0, 0]);
    assert_eq!(text.data, vec![b'A', 0xDF]);
    let ntext = encode_ntext(&Value::Text("Aß".to_owned()), -1).unwrap();
    assert_eq!(ntext.length, vec![4, 0, 0, 0]);
    assert_eq!(ntext.data, vec![b'A', 0, 0xDF, 0]);
    assert_eq!(
        encode_image(&Value::Binary(vec![1, 2]), -1).unwrap().data,
        vec![1, 2]
    );
    let xml = encode_xml(&Value::Text("x".to_owned())).unwrap();
    assert_eq!(xml.type_info, vec![0xF1, 0]);
    assert_eq!(
        xml.length,
        vec![0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
}

#[test]
fn encodes_null_legacy_lob_rpc_fields_like_node() {
    let mut collation = Collation::new(0x0409, 0, 0, 0);
    let expected_length = vec![0xFF, 0xFF, 0xFF, 0xFF];

    let text = encode_text(&Value::Null, -1, &collation).unwrap();
    assert_eq!(
        text.type_info,
        [
            vec![0x23, 0xFF, 0xFF, 0xFF, 0xFF],
            collation.to_buffer().to_vec()
        ]
        .concat()
    );
    assert_eq!(text.length, expected_length);
    assert!(text.data.is_empty());

    let ntext = encode_ntext_with_collation(&Value::Null, -1, &collation).unwrap();
    assert_eq!(
        ntext.type_info,
        [
            vec![0x63, 0xFF, 0xFF, 0xFF, 0xFF],
            collation.to_buffer().to_vec()
        ]
        .concat()
    );
    assert_eq!(ntext.length, expected_length);
    assert!(ntext.data.is_empty());

    let image = encode_image(&Value::Null, -1).unwrap();
    assert_eq!(image.type_info, vec![0x22, 0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(image.length, expected_length);
    assert!(image.data.is_empty());
}

#[test]
fn legacy_lob_type_lengths_follow_node_resolve_length_and_preserve_ntext_collation() {
    let mut collation = Collation::new(0x0409, 0x80000000, 0, 0);
    let text = tedious_rs::parameter::SqlParameterType::Text
        .encode(
            &Value::Text("A".to_owned()),
            &tedious_rs::parameter::ParameterOptions {
                collation: Some(collation.clone()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(&text.type_info[1..5], &[1, 0, 0, 0]);
    assert_eq!(&text.type_info[5..10], &collation.to_buffer());

    let ntext = tedious_rs::parameter::SqlParameterType::NText
        .encode(
            &Value::Text("😀".to_owned()),
            &tedious_rs::parameter::ParameterOptions {
                collation: Some(collation.clone()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(&ntext.type_info[1..5], &[2, 0, 0, 0]);
    assert_eq!(&ntext.type_info[5..10], &collation.to_buffer());
    assert_eq!(ntext.length, vec![4, 0, 0, 0]);
}

#[test]
fn legacy_lob_lengths_reject_values_that_cannot_be_written_to_int32() {
    let options = tedious_rs::parameter::ParameterOptions {
        length: Some(i32::MAX as usize + 1),
        ..Default::default()
    };
    assert!(
        tedious_rs::parameter::SqlParameterType::Text
            .encode(&Value::Text("x".to_owned()), &options)
            .is_err()
    );
    assert!(
        tedious_rs::parameter::SqlParameterType::NText
            .encode(&Value::Text("x".to_owned()), &options)
            .is_err()
    );
    assert!(
        tedious_rs::parameter::SqlParameterType::Image
            .encode(&Value::Binary(vec![1]), &options)
            .is_err()
    );
}

#[test]
fn datetime_rounding_rolls_midnight_into_the_next_day() {
    let value = Value::DateTime {
        date: Box::new(Value::Date {
            year: 1900,
            month: 1,
            day: 1,
        }),
        time: Box::new(Value::Time {
            hour: 23,
            minute: 59,
            second: 59,
            nanosecond: 999_999_999,
        }),
    };
    let encoded = encode_datetime(&value).unwrap();
    assert_eq!(encoded.data, vec![1, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn encodes_char_and_nchar_with_node_value_lengths() {
    let collation = Collation::new(0x409, 0, 0, 0);
    let char_value = encode_char(&Value::Text("ab".to_owned()), 4, &collation).unwrap();
    assert_eq!(char_value.type_info[0], 0xAF);
    assert_eq!(char_value.length, vec![2, 0]);
    assert_eq!(char_value.data, b"ab");
    let nchar_value = encode_nchar(&Value::Text("ab".to_owned()), 4).unwrap();
    assert_eq!(nchar_value.type_info[0], 0xEF);
    assert_eq!(nchar_value.length, vec![4, 0]);
    assert_eq!(
        nchar_value.data,
        "ab".encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
}

#[test]
fn rejects_non_plp_string_and_binary_lengths_that_exceed_tds_prefixes() {
    let collation = Collation::new(0x0409, 0, 0, 0);
    assert!(encode_varchar(&Value::Text("a".repeat(70_000)), 8000, &collation).is_err());
    assert!(encode_nvarchar(&Value::Text("a".repeat(40_000)), 4000).is_err());
    assert!(encode_varbinary(&Value::Binary(vec![0; 70_000]), 8000).is_err());
}

#[test]
fn encodes_bit_using_node_truthiness_for_primitive_values() {
    assert_eq!(encode_bit(&Value::Integer(0)).unwrap().data, vec![0]);
    assert_eq!(encode_bit(&Value::Integer(1)).unwrap().data, vec![1]);
    assert_eq!(
        encode_bit(&Value::Text("0".to_owned())).unwrap().data,
        vec![1]
    );
    assert_eq!(
        encode_bit(&Value::Text(String::new())).unwrap().data,
        vec![0]
    );
    assert_eq!(encode_bit(&Value::Binary(vec![1])).unwrap().data, vec![1]);
}

#[test]
fn encodes_big_integer_to_bit_using_bigint_truthiness() {
    assert_eq!(
        encode_bit(&Value::BigInteger("0".to_owned())).unwrap().data,
        vec![0]
    );
    assert_eq!(
        encode_bit(&Value::BigInteger("-0".to_owned()))
            .unwrap()
            .data,
        vec![0]
    );
    assert_eq!(
        encode_bit(&Value::BigInteger("1".to_owned())).unwrap().data,
        vec![1]
    );
    assert_eq!(
        encode_bit(&Value::BigInteger("-1".to_owned()))
            .unwrap()
            .data,
        vec![1]
    );
    assert!(encode_bit(&Value::BigInteger("+0x1".to_owned())).is_err());
}
