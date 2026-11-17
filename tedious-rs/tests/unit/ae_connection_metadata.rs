use tedious_rs::token::rows::parse_colmetadata;

fn bvar(value: &str) -> Vec<u8> {
    let units = value.encode_utf16().collect::<Vec<_>>();
    [
        vec![units.len() as u8],
        units.into_iter().flat_map(u16::to_le_bytes).collect(),
    ]
    .concat()
}

#[test]
fn parses_column_encryption_descriptor_after_column_name() {
    let mut wire = vec![1, 0, 0, 0, 0, 0, 0x00, 0x08, 0x38];
    wire.extend(bvar("Secret"));
    wire.extend([3, 0, 2, 1, 0]);

    let parsed = parse_colmetadata(&wire, "7_4", false).unwrap();
    let encryption = parsed.value[0].crypto_metadata.as_ref().unwrap();
    assert_eq!(encryption.ordinal, 3);
    assert_eq!(encryption.cipher_algorithm_id, 2);
    assert_eq!(encryption.encryption_type as u8, 1);
    assert_eq!(encryption.normalization_rule_version, vec![0]);
    assert_eq!(parsed.offset, wire.len());
}

#[test]
fn leaves_unmarked_column_metadata_without_encryption_descriptor() {
    let mut wire = vec![1, 0, 0, 0, 0, 0, 0, 0, 0x38];
    wire.extend(bvar("Plain"));

    let parsed = parse_colmetadata(&wire, "7_4", false).unwrap();
    assert!(parsed.value[0].crypto_metadata.is_none());
    assert_eq!(parsed.offset, wire.len());
}
