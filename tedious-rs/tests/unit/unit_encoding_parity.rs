use tedious_rs::collation::Collation;
use tedious_rs::value_codec::{Value, encode_text, encode_varchar};

fn collation(language_id: u32) -> Collation {
    Collation::new(language_id, 0, 0, 0)
}

fn varchar_bytes(value: &str, language_id: u32) -> Vec<u8> {
    encode_varchar(
        &Value::Text(value.to_owned()),
        8000,
        &collation(language_id),
    )
    .expect("varchar encoding must succeed")
    .data
}

#[test]
fn single_byte_codepages_replace_each_unrepresentable_character_with_question_mark() {
    let cases = [
        (0x0401, "A😀Ж", vec![b'A', b'?', b'?']),
        (0x0405, "A😀Ж", vec![b'A', b'?', b'?']),
        (0x0409, "A😀Ж", vec![b'A', b'?', b'?']),
        (0x0419, "A😀Ж", vec![b'A', b'?', 0xC6]),
        (0x0411, "A😀漢", vec![b'A', b'?', 0x8A, 0xBF]),
    ];

    for (language_id, value, expected) in cases {
        assert_eq!(varchar_bytes(value, language_id), expected);
    }
}

#[test]
fn yore_codepages_preserve_representable_bytes_and_replace_unrepresentable_characters() {
    let cp437 = Collation::new(0, 0, 0, 30);
    let cp850 = Collation::new(0, 0, 0, 40);

    assert_eq!(
        encode_varchar(&Value::Text("Aé😀".to_owned()), 8000, &cp437)
            .expect("cp437 encoding must succeed")
            .data,
        vec![b'A', 0x82, b'?']
    );
    assert_eq!(
        encode_varchar(&Value::Text("Aé😀".to_owned()), 8000, &cp850)
            .expect("cp850 encoding must succeed")
            .data,
        vec![b'A', 0x82, b'?']
    );
}

#[test]
fn multibyte_codepages_replace_unrepresentable_codepoints_without_dropping_following_text() {
    assert_eq!(
        varchar_bytes("A漢😀B", 0x0411),
        vec![b'A', 0x8A, 0xBF, b'?', b'B']
    );
    assert_eq!(
        varchar_bytes("A漢😀B", 0x0804),
        vec![b'A', 0x9D, 0x68, b'?', b'B']
    );
    assert_eq!(
        varchar_bytes("A한😀B", 0x0412),
        vec![b'A', 0xC7, 0xD1, b'?', b'B']
    );
    assert_eq!(
        varchar_bytes("A漢😀B", 0x0404),
        vec![b'A', 0xBA, 0x7E, b'?', b'B']
    );
}

#[test]
fn cp949_matches_iconv_lite_windows_949_table() {
    assert_eq!(varchar_bytes("漢", 0x0412), vec![0xF9, 0xD3]);
    assert_eq!(varchar_bytes("한", 0x0412), vec![0xC7, 0xD1]);
    assert_eq!(varchar_bytes("😀", 0x0412), vec![b'?']);
}

#[test]
fn cp949_fallback_preserves_following_text() {
    assert_eq!(
        varchar_bytes("A漢😀한B", 0x0412),
        vec![b'A', 0xF9, 0xD3, b'?', 0xC7, 0xD1, b'B']
    );
}

#[test]
fn text_uses_the_same_fallback_bytes_as_varchar() {
    let value = Value::Text("prefix😀suffix".to_owned());
    let c = collation(0x0409);
    let varchar = encode_varchar(&value, 8000, &c)
        .expect("varchar encoding must succeed")
        .data;
    let text = encode_text(&value, -1, &c)
        .expect("text encoding must succeed")
        .data;
    assert_eq!(text, varchar);
}

#[test]
fn null_encoding_remains_null_and_does_not_emit_a_fallback_byte() {
    let encoded = encode_varchar(&Value::Null, 8000, &collation(0x0409))
        .expect("null varchar encoding must succeed");
    assert_eq!(encoded.length, vec![0xFF, 0xFF]);
    assert!(encoded.data.is_empty());
}
