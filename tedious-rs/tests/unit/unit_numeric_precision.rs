use tedious_rs::collation::Collation;
use tedious_rs::value_codec::{Value, encode_bigint, encode_decimal, encode_int, encode_varchar};

fn magnitude(data: &[u8]) -> i128 {
    let mut bytes = [0_u8; 16];
    bytes[..data.len() - 1].copy_from_slice(&data[1..]);
    let value = i128::from_le_bytes(bytes);
    if data[0] == 0 { -value } else { value }
}

#[test]
fn preserves_integer_text_beyond_javascript_safe_integer_range() {
    let encoded = encode_decimal(&Value::Text("9007199254740993.00".to_owned()), 20, 2).unwrap();
    assert_eq!(magnitude(&encoded.data), 900719925474099300);
    assert_eq!(encoded.type_info, vec![0x6A, 13, 20, 2]);
}

#[test]
fn preserves_big_integer_text_without_floating_point_conversion() {
    let encoded = encode_decimal(&Value::BigInteger("9007199254740993".to_owned()), 19, 0).unwrap();
    assert_eq!(magnitude(&encoded.data), 9007199254740993);
    assert_eq!(
        encode_bigint(&Value::BigInteger("9223372036854775807".to_owned()))
            .unwrap()
            .data,
        i64::MAX.to_le_bytes()
    );
}

#[test]
fn preserves_maximum_supported_decimal_precision_and_scale() {
    let maximum = encode_decimal(
        &Value::Text("99999999999999999999999999999999999999".to_owned()),
        38,
        0,
    )
    .unwrap();
    assert_eq!(
        magnitude(&maximum.data),
        99999999999999999999999999999999999999
    );

    let smallest = encode_decimal(
        &Value::Text("0.00000000000000000000000000000000000001".to_owned()),
        38,
        38,
    )
    .unwrap();
    assert_eq!(magnitude(&smallest.data), 1);
}

#[test]
fn rejects_decimal_scale_reduction_that_would_lose_digits() {
    let result = encode_decimal(
        &Value::Decimal {
            unscaled: 12345,
            scale: 3,
        },
        9,
        2,
    );
    assert!(result.is_err());
}

#[test]
fn integer_coercion_does_not_round_decimal_text() {
    assert_eq!(
        encode_int(&Value::Text("2147483647.9".to_owned()))
            .unwrap()
            .data,
        i32::MAX.to_le_bytes()
    );
    assert_eq!(
        encode_int(&Value::Text("-2147483648.9".to_owned()))
            .unwrap()
            .data,
        i32::MIN.to_le_bytes()
    );
}

#[test]
fn preserves_node_decimal_prefix_and_rounding_without_f64() {
    let positive = encode_decimal(&Value::Text("12.345tail".to_owned()), 9, 2).unwrap();
    assert_eq!(magnitude(&positive.data), 1235);
    let negative = encode_decimal(&Value::Text("-12.345tail".to_owned()), 9, 2).unwrap();
    assert_eq!(magnitude(&negative.data), -1234);
}

#[test]
fn matches_iconv_lite_oem_and_multibyte_codepage_fixtures() {
    let cp437 = Collation::new(0x0409, 0, 0, 30);
    let cp850 = Collation::new(0x0409, 0, 0, 40);
    let cp932 = Collation::new(0x0411, 0, 0, 0);
    let cp936 = Collation::new(0x0804, 0, 0, 0);

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
        encode_varchar(&Value::Text("日本".to_owned()), 4, &cp932)
            .unwrap()
            .data,
        vec![0x93, 0xFA, 0x96, 0x7B]
    );
    assert_eq!(
        encode_varchar(&Value::Text("中文".to_owned()), 4, &cp936)
            .unwrap()
            .data,
        vec![0xD6, 0xD0, 0xCE, 0xC4]
    );
}
