use tedious_rs::collation::{Collation, Encoding, UTF8};

#[test]
fn parses_and_serializes_collation_wire_fields() {
    let mut collation = Collation::from_buffer(&[0x09, 0x04, 0x00, 0x00, 0x00], 0).unwrap();
    assert_eq!(collation.lcid, 0x0409);
    assert_eq!(collation.flags, 0);
    assert_eq!(collation.version, 0);
    assert_eq!(collation.sort_id, 0);
    assert_eq!(collation.codepage, Some(Encoding::Cp1252));
    assert_eq!(collation.to_buffer(), [0x09, 0x04, 0x00, 0x00, 0x00]);
}

#[test]
fn utf8_flag_precedes_sort_id_and_language_selection() {
    let mut collation = Collation::new(0x0409, UTF8, 2, 52);
    assert_eq!(collation.codepage, Some(Encoding::Utf8));
    assert_eq!(collation.to_buffer(), [0x09, 0x04, 0x00, 0x24, 52]);
    assert!(Collation::from_buffer(&[1, 2, 3, 4], 0).is_none());
}
