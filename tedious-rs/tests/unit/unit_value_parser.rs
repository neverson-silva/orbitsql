use tedious_rs::metadata_parser::read_metadata;
use tedious_rs::value_codec::Value;
use tedious_rs::value_parser::{
    is_plp_stream, read_plp_stream, read_value, read_value_with_options,
};

fn metadata(id: u8, extra: &[u8]) -> tedious_rs::metadata_parser::Metadata {
    let mut bytes = vec![0, 0, 0, 0, 0, 0, id];
    bytes.extend_from_slice(extra);
    read_metadata(&bytes, 0, "7_4").unwrap().value
}

#[test]
fn reads_fixed_values_with_exact_offsets() {
    let int = metadata(0x38, &[]);
    let result = read_value(&[0xFE, 0xFF, 0xFF, 0xFF, 9], 0, &int).unwrap();
    assert_eq!(result.value, Value::Integer(-2));
    assert_eq!(result.offset, 4);

    let big = metadata(0x7F, &[]);
    let result = read_value(&[0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF], 0, &big).unwrap();
    assert_eq!(result.value, Value::BigInteger("-2".to_owned()));
}

#[test]
fn reads_n_variants_and_null_lengths() {
    let int = metadata(0x26, &[4]);
    let result = read_value(&[4, 1, 0, 0, 0], 0, &int).unwrap();
    assert_eq!(result.value, Value::Integer(1));
    assert_eq!(result.offset, 5);

    let null = read_value(&[0], 0, &metadata(0x68, &[1])).unwrap();
    assert_eq!(null.value, Value::Null);
    assert_eq!(null.offset, 1);
}

#[test]
fn reads_sql_variant_scalar_and_binary_values() {
    let variant = metadata(0x62, &[0, 0, 0, 0]);
    let mut scalar = vec![6, 0, 0, 0, 0x38, 0];
    scalar.extend_from_slice(&42i32.to_le_bytes());
    let parsed = read_value(&scalar, 0, &variant).unwrap();
    assert_eq!(parsed.value, Value::Integer(42));
    assert_eq!(parsed.offset, scalar.len());

    let mut binary = vec![7, 0, 0, 0, 0xA5, 2, 3, 0];
    binary.extend_from_slice(&[1, 2, 3]);
    let parsed = read_value(&binary, 0, &variant).unwrap();
    assert_eq!(parsed.value, Value::Binary(vec![1, 2, 3]));
    assert_eq!(parsed.offset, binary.len());
}

#[test]
fn reads_sql_variant_temporal_values_without_nullable_length_prefixes() {
    let variant = metadata(0x62, &[0, 0, 0, 0]);

    let mut date = vec![5, 0, 0, 0, 0x28, 0];
    date.extend_from_slice(&719162_u32.to_le_bytes()[..3]);
    let parsed = read_value(&date, 0, &variant).unwrap();
    assert_eq!(
        parsed.value,
        Value::Date {
            year: 1970,
            month: 1,
            day: 1
        }
    );
    assert_eq!(parsed.offset, date.len());

    let mut time = vec![6, 0, 0, 0, 0x29, 1, 1];
    time.extend_from_slice(&[0, 0, 0]);
    let parsed = read_value(&time, 0, &variant).unwrap();
    assert_eq!(
        parsed.value,
        Value::Time {
            hour: 0,
            minute: 0,
            second: 0,
            nanosecond: 0
        }
    );
    assert_eq!(parsed.offset, time.len());

    let mut datetime2 = vec![9, 0, 0, 0, 0x2A, 1, 1];
    datetime2.extend_from_slice(&[0, 0, 0]);
    datetime2.extend_from_slice(&719162_u32.to_le_bytes()[..3]);
    let parsed = read_value(&datetime2, 0, &variant).unwrap();
    assert!(matches!(parsed.value, Value::DateTime { .. }));
    assert_eq!(parsed.offset, datetime2.len());

    let mut datetime_offset = vec![11, 0, 0, 0, 0x2B, 1, 1];
    datetime_offset.extend_from_slice(&[0, 0, 0]);
    datetime_offset.extend_from_slice(&719162_u32.to_le_bytes()[..3]);
    datetime_offset.extend_from_slice(&60_i16.to_le_bytes());
    let parsed = read_value(&datetime_offset, 0, &variant).unwrap();
    assert!(matches!(
        parsed.value,
        Value::DateTimeOffset {
            offset_minutes: 60,
            ..
        }
    ));
    assert_eq!(parsed.offset, datetime_offset.len());
}

#[test]
fn reads_binary_unicode_and_collated_text() {
    let binary = metadata(0xA5, &[4, 0]);
    assert_eq!(
        read_value(&[3, 0, 1, 2, 3], 0, &binary).unwrap().value,
        Value::Binary(vec![1, 2, 3])
    );

    let unicode = metadata(0xE7, &[4, 0, 9, 4, 0, 0, 52]);
    assert_eq!(
        read_value(&[4, 0, 0x41, 0, 0xDF, 0], 0, &unicode)
            .unwrap()
            .value,
        Value::Text("Aß".to_owned())
    );

    let text = metadata(0xA7, &[3, 0, 9, 4, 0, 0, 52]);
    assert_eq!(
        read_value(&[3, 0, 0x41, 0xDF, 0x21], 0, &text)
            .unwrap()
            .value,
        Value::Text("Aß!".to_owned())
    );
}

#[test]
fn reads_udt_values_as_plp_binary_and_preserves_nulls() {
    let mut metadata_bytes = vec![
        0, 0, 0, 0, 0, 0, 0xF0, 0x20, 0x00, 2, 0x64, 0x00, 0x62, 0x00,
    ];
    metadata_bytes.extend([3, 0x64, 0x00, 0x62, 0x00, 0x6F, 0x00]);
    metadata_bytes.extend([
        5, 0x50, 0x00, 0x6F, 0x00, 0x69, 0x00, 0x6E, 0x00, 0x74, 0x00,
    ]);
    metadata_bytes.extend([
        8, 0, 0x41, 0x00, 0x73, 0x00, 0x73, 0x00, 0x65, 0x00, 0x6D, 0x00, 0x62, 0x00, 0x6C, 0x00,
        0x79, 0x00,
    ]);
    let udt = read_metadata(&metadata_bytes, 0, "7_4").unwrap().value;
    let mut value = 3_u64.to_le_bytes().to_vec();
    value.extend(3_u32.to_le_bytes());
    value.extend([1, 2, 3]);
    value.extend(0_u32.to_le_bytes());
    let parsed = read_value(&value, 0, &udt).unwrap();
    assert_eq!(parsed.value, Value::Binary(vec![1, 2, 3]));
    assert_eq!(parsed.offset, value.len());

    let null = u64::MAX.to_le_bytes().to_vec();
    let parsed = read_value(&null, 0, &udt).unwrap();
    assert_eq!(parsed.value, Value::Null);
    assert_eq!(parsed.offset, null.len());
}

#[test]
fn decodes_oem_collated_text_supported_by_iconv_lite() {
    let cp437 = metadata(0xA7, &[1, 0, 9, 4, 0, 0, 30]);
    assert_eq!(
        read_value(&[1, 0, 0x82], 0, &cp437).unwrap().value,
        Value::Text("é".to_owned())
    );
    let cp850 = metadata(0xA7, &[1, 0, 9, 4, 0, 0, 40]);
    assert_eq!(
        read_value(&[1, 0, 0x82], 0, &cp850).unwrap().value,
        Value::Text("é".to_owned())
    );
    let cp949 = metadata(0xA7, &[4, 0, 0x12, 0x04, 0, 0, 0]);
    assert_eq!(
        read_value(&[4, 0, 0xC7, 0xD1, 0xB1, 0xDB], 0, &cp949)
            .unwrap()
            .value,
        Value::Text("한글".to_owned())
    );
}

#[test]
fn falls_back_to_utf8_for_unknown_collation_codepage() {
    let unknown = metadata(0xA7, &[4, 0, 0, 0, 0, 0, 0]);
    let value = [4, 0, 0xC3, 0x28, 0xE2, 0x82, 0x7F];
    let parsed = read_value(&value, 0, &unknown).unwrap();
    assert_eq!(parsed.value, Value::Text("�(�".to_owned()));
    assert_eq!(parsed.offset, 6);

    let truncated = [5, 0, 0xC3, 0x28, 0xE2, 0x82];
    assert!(read_value(&truncated, 0, &unknown).is_err());
}

#[test]
fn rejects_truncated_and_unsupported_value_payloads() {
    let int = metadata(0x38, &[]);
    assert!(read_value(&[1, 2], 0, &int).is_err());
    let unknown = metadata(0x3C, &[]);
    assert!(read_value(&[1], 0, &unknown).is_err());
}

#[test]
fn reads_temporal_and_decimal_values_from_wire() {
    let date = metadata(0x28, &[]);
    let mut date_bytes = vec![3];
    date_bytes.extend_from_slice(&719162_u32.to_le_bytes()[..3]);
    assert_eq!(
        read_value(&date_bytes, 0, &date).unwrap().value,
        Value::Date {
            year: 1970,
            month: 1,
            day: 1
        }
    );

    let time = metadata(0x29, &[3]);
    let mut ticks = vec![4];
    ticks.extend_from_slice(&3_723_400_u32.to_le_bytes());
    assert_eq!(
        read_value(&ticks, 0, &time).unwrap().value,
        Value::Time {
            hour: 1,
            minute: 2,
            second: 3,
            nanosecond: 400_000_000
        }
    );

    let datetime = metadata(0x2A, &[7]);
    let mut datetime_bytes = vec![8];
    datetime_bytes.extend([0; 8]);
    datetime_bytes[6..9].copy_from_slice(&719162_u32.to_le_bytes()[..3]);
    assert_eq!(
        read_value(&datetime_bytes, 0, &datetime).unwrap().value,
        Value::DateTime {
            date: Box::new(Value::Date {
                year: 1970,
                month: 1,
                day: 1
            }),
            time: Box::new(Value::Time {
                hour: 0,
                minute: 0,
                second: 0,
                nanosecond: 0
            })
        }
    );
    assert_eq!(read_value(&[0], 0, &date).unwrap().value, Value::Null);
    assert_eq!(read_value(&[0], 0, &time).unwrap().value, Value::Null);
    assert_eq!(read_value(&[0], 0, &datetime).unwrap().value, Value::Null);

    let decimal = metadata(0x6A, &[5, 9, 2]);
    assert_eq!(
        read_value(&[5, 0, 0x39, 0x30, 0, 0], 0, &decimal)
            .unwrap()
            .value,
        Value::Decimal {
            unscaled: -12345,
            scale: 2
        }
    );
    assert_eq!(
        read_value(&[5, 2, 0x39, 0x30, 0, 0], 0, &decimal)
            .unwrap()
            .value,
        Value::Decimal {
            unscaled: -12345,
            scale: 2
        }
    );
}

#[test]
fn preserves_signed_decimal_i128_boundaries_and_rejects_unrepresentable_magnitudes() {
    let decimal = metadata(0x6A, &[17, 38, 0]);

    let positive_max = i128::MAX.to_le_bytes();
    let mut positive = vec![17, 1];
    positive.extend_from_slice(&positive_max);
    assert_eq!(
        read_value(&positive, 0, &decimal).unwrap().value,
        Value::Decimal {
            unscaled: i128::MAX,
            scale: 0
        }
    );

    let negative_min = i128::MIN.to_le_bytes();
    let mut negative = vec![17, 0];
    negative.extend_from_slice(&negative_min);
    assert_eq!(
        read_value(&negative, 0, &decimal).unwrap().value,
        Value::Decimal {
            unscaled: i128::MIN,
            scale: 0
        }
    );

    let mut positive_overflow = vec![17, 1];
    positive_overflow.extend_from_slice(&u128::MAX.to_le_bytes());
    assert!(matches!(
        read_value(&positive_overflow, 0, &decimal),
        Err(tedious_rs::value_parser::ValueParserError::Unsupported(message))
            if message.contains("positive range")
    ));

    let mut negative_overflow = vec![17, 0];
    negative_overflow.extend_from_slice(&u128::MAX.to_le_bytes());
    assert!(matches!(
        read_value(&negative_overflow, 0, &decimal),
        Err(tedious_rs::value_parser::ValueParserError::Unsupported(message))
            if message.contains("negative range")
    ));
}

#[test]
fn reads_nullable_datetimeoffset_with_scale_length_and_rejects_mismatch() {
    let offset = metadata(0x2B, &[7]);
    let mut bytes = vec![10];
    bytes.extend([0, 0, 0, 0, 0]);
    bytes.extend([0, 0, 0]);
    bytes.extend_from_slice(&(-180_i16).to_le_bytes());
    let parsed = read_value(&bytes, 0, &offset).unwrap();
    assert_eq!(
        parsed.value,
        Value::DateTimeOffset {
            date: Box::new(Value::Date {
                year: 1,
                month: 1,
                day: 1
            }),
            time: Box::new(Value::Time {
                hour: 0,
                minute: 0,
                second: 0,
                nanosecond: 0
            }),
            offset_minutes: -180
        }
    );
    assert_eq!(parsed.offset, bytes.len());
    assert_eq!(read_value(&[0], 0, &offset).unwrap().value, Value::Null);
    assert!(read_value(&[9, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0, &offset).is_err());
}

#[test]
fn preserves_local_temporal_representation_when_use_utc_is_disabled() {
    let date = metadata(0x28, &[]);
    let parsed = read_value_with_options(&[3, 0x07, 0x24, 0x0B], 0, &date, false).unwrap();
    assert_eq!(
        parsed.value,
        Value::LocalDate {
            year: 2000,
            month: 1,
            day: 1
        }
    );

    let time = metadata(0x29, &[3]);
    let mut bytes = vec![4];
    bytes.extend_from_slice(&3_723_400_u32.to_le_bytes());
    let parsed = read_value_with_options(&bytes, 0, &time, false).unwrap();
    assert_eq!(
        parsed.value,
        Value::LocalTime {
            hour: 1,
            minute: 2,
            second: 3,
            nanosecond: 400_000_000
        }
    );
}

#[test]
fn reads_legacy_datetime_and_money_values() {
    let datetime = metadata(0x3D, &[]);
    let bytes = vec![0, 0, 0, 0, 0xC0, 0x7A, 0x10, 0];
    assert_eq!(
        read_value(&bytes, 0, &datetime).unwrap().value,
        Value::DateTime {
            date: Box::new(Value::Date {
                year: 1900,
                month: 1,
                day: 1
            }),
            time: Box::new(Value::Time {
                hour: 1,
                minute: 0,
                second: 0,
                nanosecond: 0
            })
        }
    );
    let money = metadata(0x3C, &[]);
    let bytes = (-123456i64).to_le_bytes().to_vec();
    assert_eq!(
        read_value(&bytes, 0, &money).unwrap().value,
        Value::Decimal {
            unscaled: -123456,
            scale: 4
        }
    );
}

#[test]
fn rounds_legacy_datetime_ticks_like_node_dates() {
    let datetime = metadata(0x3D, &[]);
    let mut bytes = 0_i32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&299_i32.to_le_bytes());
    assert_eq!(
        read_value(&bytes, 0, &datetime).unwrap().value,
        Value::DateTime {
            date: Box::new(Value::Date {
                year: 1900,
                month: 1,
                day: 1
            }),
            time: Box::new(Value::Time {
                hour: 0,
                minute: 0,
                second: 0,
                nanosecond: 997_000_000
            })
        }
    );

    let mut boundary = 0_i32.to_le_bytes().to_vec();
    boundary.extend_from_slice(&25_920_000_i32.to_le_bytes());
    let parsed = read_value(&boundary, 0, &datetime).unwrap().value;
    assert_eq!(
        parsed,
        Value::DateTime {
            date: Box::new(Value::Date {
                year: 1900,
                month: 1,
                day: 2
            }),
            time: Box::new(Value::Time {
                hour: 0,
                minute: 0,
                second: 0,
                nanosecond: 0
            })
        }
    );
}

#[test]
fn reads_nullable_datetime_and_money_values() {
    let datetime = metadata(0x6F, &[8]);
    let mut datetime_bytes = vec![8];
    datetime_bytes.extend_from_slice(&0_i32.to_le_bytes());
    datetime_bytes.extend_from_slice(&1_080_000_i32.to_le_bytes());
    assert_eq!(
        read_value(&datetime_bytes, 0, &datetime).unwrap().value,
        Value::DateTime {
            date: Box::new(Value::Date {
                year: 1900,
                month: 1,
                day: 1
            }),
            time: Box::new(Value::Time {
                hour: 1,
                minute: 0,
                second: 0,
                nanosecond: 0
            })
        }
    );

    let small_datetime = metadata(0x6F, &[4]);
    assert_eq!(
        read_value(&[4, 0, 0, 60, 0], 0, &small_datetime)
            .unwrap()
            .value,
        Value::DateTime {
            date: Box::new(Value::Date {
                year: 1900,
                month: 1,
                day: 1
            }),
            time: Box::new(Value::Time {
                hour: 1,
                minute: 0,
                second: 0,
                nanosecond: 0
            })
        }
    );

    let money = metadata(0x6E, &[8]);
    let mut money_bytes = vec![8];
    money_bytes.extend_from_slice(&(-123456_i64).to_le_bytes());
    assert_eq!(
        read_value(&money_bytes, 0, &money).unwrap().value,
        Value::Decimal {
            unscaled: -123456,
            scale: 4
        }
    );
    assert_eq!(read_value(&[0], 0, &money).unwrap().value, Value::Null);
}

#[test]
fn rejects_invalid_nullable_datetime_and_money_lengths() {
    assert!(read_value(&[3, 0, 0, 0], 0, &metadata(0x6F, &[8])).is_err());
    assert!(read_value(&[2, 0, 0], 0, &metadata(0x6E, &[8])).is_err());
}

#[test]
fn reads_plp_null_and_chunked_values_without_confusing_them_with_short_values() {
    let binary = metadata(0xA5, &[0xFF, 0xFF]);
    assert!(is_plp_stream(&binary));
    let mut plp = u64::from(3_u8).to_le_bytes().to_vec();
    plp.extend([3, 0, 0, 0, 1, 2, 3, 0, 0, 0, 0]);
    let parsed = read_value(&plp, 0, &binary).unwrap();
    assert_eq!(parsed.value, Value::Binary(vec![1, 2, 3]));
    assert_eq!(parsed.offset, plp.len());
    let chunks = read_plp_stream(&plp, 0).unwrap();
    assert_eq!(chunks.value, Some(vec![vec![1, 2, 3]]));
    let null = u64::MAX.to_le_bytes().to_vec();
    assert_eq!(read_value(&null, 0, &binary).unwrap().value, Value::Null);

    let unicode = metadata(0xE7, &[0xFF, 0xFF, 9, 4, 0, 0, 52]);
    let mut unicode_plp = u64::from(4_u8).to_le_bytes().to_vec();
    unicode_plp.extend([4, 0, 0, 0, b'A', 0, 0xDF, 0, 0, 0, 0, 0]);
    assert_eq!(
        read_value(&unicode_plp, 0, &unicode).unwrap().value,
        Value::Text("Aß".to_owned())
    );
    assert!(is_plp_stream(&unicode));
}

#[test]
fn reads_unknown_length_plp_values_until_the_terminator_for_all_max_types() {
    let unknown = 0xFFFFFFFFFFFFFFFE_u64.to_le_bytes();
    let mut binary = unknown.to_vec();
    binary.extend_from_slice(&3_u32.to_le_bytes());
    binary.extend_from_slice(&[1, 2, 3]);
    binary.extend_from_slice(&0_u32.to_le_bytes());
    let binary_metadata = metadata(0xA5, &[0xFF, 0xFF]);
    assert_eq!(
        read_value(&binary, 0, &binary_metadata).unwrap().value,
        Value::Binary(vec![1, 2, 3])
    );

    let mut varchar = unknown.to_vec();
    varchar.extend_from_slice(&2_u32.to_le_bytes());
    varchar.extend_from_slice(b"ok");
    varchar.extend_from_slice(&0_u32.to_le_bytes());
    let varchar_metadata = metadata(0xA7, &[0xFF, 0xFF, 9, 4, 0, 0, 52]);
    assert_eq!(
        read_value(&varchar, 0, &varchar_metadata).unwrap().value,
        Value::Text("ok".to_owned())
    );

    let mut nvarchar = unknown.to_vec();
    nvarchar.extend_from_slice(&4_u32.to_le_bytes());
    nvarchar.extend_from_slice(&[b'o', 0, b'k', 0]);
    nvarchar.extend_from_slice(&0_u32.to_le_bytes());
    let nvarchar_metadata = metadata(0xE7, &[0xFF, 0xFF, 9, 4, 0, 0, 52]);
    assert_eq!(
        read_value(&nvarchar, 0, &nvarchar_metadata).unwrap().value,
        Value::Text("ok".to_owned())
    );

    let mut xml = unknown.to_vec();
    xml.extend_from_slice(&4_u32.to_le_bytes());
    xml.extend_from_slice(&[b'o', 0, b'k', 0]);
    xml.extend_from_slice(&0_u32.to_le_bytes());
    let xml_metadata = metadata(0xF1, &[0]);
    assert_eq!(
        read_value(&xml, 0, &xml_metadata).unwrap().value,
        Value::Text("ok".to_owned())
    );

    let mut udt = unknown.to_vec();
    udt.extend_from_slice(&3_u32.to_le_bytes());
    udt.extend_from_slice(&[1, 2, 3]);
    udt.extend_from_slice(&0_u32.to_le_bytes());
    let udt_metadata = metadata(0xF0, &[0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        read_value(&udt, 0, &udt_metadata).unwrap().value,
        Value::Binary(vec![1, 2, 3])
    );
}

#[test]
fn replaces_invalid_collated_bytes_like_iconv_lite() {
    let cp1252 = metadata(0xA7, &[1, 0, 9, 4, 0, 0, 52]);
    let parsed = read_value(&[1, 0, 0x81], 0, &cp1252).unwrap();
    assert_eq!(parsed.value, Value::Text("�".to_owned()));
}

#[test]
fn identifies_only_node_plp_metadata_as_plp_streams() {
    assert!(!is_plp_stream(&metadata(0xA5, &[8, 0])));
    assert!(is_plp_stream(&metadata(0xF1, &[0])));
    assert!(!is_plp_stream(&metadata(0x38, &[])));
}

#[test]
fn rejects_plp_values_with_a_known_length_that_does_not_match_chunks() {
    let binary = metadata(0xA5, &[0xFF, 0xFF]);
    let mut plp = 4_u64.to_le_bytes().to_vec();
    plp.extend([3, 0, 0, 0, 1, 2, 3, 0, 0, 0]);
    assert!(read_value(&plp, 0, &binary).is_err());
}

#[test]
fn reads_legacy_lob_lengths_and_null_markers() {
    let text = metadata(0x23, &[4, 0, 0, 0, 9, 4, 0, 0, 52]);
    let mut text_bytes = vec![2, 0xAA, 0xBB];
    text_bytes.extend([0; 8]);
    text_bytes.extend([2, 0, 0, 0, b'A', 0xDF]);
    assert_eq!(
        read_value(&text_bytes, 0, &text).unwrap().value,
        Value::Text("Aß".to_owned())
    );
    let ntext = metadata(0x63, &[4, 0, 0, 0, 9, 4, 0, 0, 52]);
    let mut ntext_bytes = vec![2, 0xAA, 0xBB];
    ntext_bytes.extend([0; 8]);
    ntext_bytes.extend([4, 0, 0, 0, b'A', 0, 0xDF, 0]);
    assert_eq!(
        read_value(&ntext_bytes, 0, &ntext).unwrap().value,
        Value::Text("Aß".to_owned())
    );
    let image = metadata(0x22, &[4, 0, 0, 0]);
    let mut image_bytes = vec![2, 0xAA, 0xBB];
    image_bytes.extend([0; 8]);
    image_bytes.extend([2, 0, 0, 0, 1, 2]);
    assert_eq!(
        read_value(&image_bytes, 0, &image).unwrap().value,
        Value::Binary(vec![1, 2])
    );
    assert_eq!(read_value(&[0], 0, &image).unwrap().value, Value::Null);
}

#[test]
fn legacy_lob_null_requires_zero_text_pointer_length() {
    let types = [
        (0x23, &[4, 0, 0, 0, 9, 4, 0, 0, 52][..]),
        (0x63, &[4, 0, 0, 0, 9, 4, 0, 0, 52][..]),
        (0x22, &[4, 0, 0, 0][..]),
    ];

    for (id, extra) in types {
        let value = metadata(id, extra);
        assert_eq!(read_value(&[0], 0, &value).unwrap().value, Value::Null);
    }

    let text = metadata(0x23, &[4, 0, 0, 0, 9, 4, 0, 0, 52]);
    let mut text_empty = vec![1, 0xAA];
    text_empty.extend([0; 8]);
    text_empty.extend([0, 0, 0, 0]);
    assert_eq!(
        read_value(&text_empty, 0, &text).unwrap().value,
        Value::Text(String::new())
    );

    let ntext = metadata(0x63, &[4, 0, 0, 0, 9, 4, 0, 0, 52]);
    let mut ntext_empty = vec![1, 0xAA];
    ntext_empty.extend([0; 8]);
    ntext_empty.extend([0, 0, 0, 0]);
    assert_eq!(
        read_value(&ntext_empty, 0, &ntext).unwrap().value,
        Value::Text(String::new())
    );

    let image = metadata(0x22, &[4, 0, 0, 0]);
    let mut image_empty = vec![1, 0xAA];
    image_empty.extend([0; 8]);
    image_empty.extend([0, 0, 0, 0]);
    assert_eq!(
        read_value(&image_empty, 0, &image).unwrap().value,
        Value::Binary(Vec::new())
    );

    let mut malformed = vec![1, 0xAA];
    malformed.extend([0; 8]);
    malformed.extend(u32::MAX.to_le_bytes());
    assert!(read_value(&malformed, 0, &image).is_err());
}
