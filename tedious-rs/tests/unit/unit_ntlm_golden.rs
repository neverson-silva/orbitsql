use tedious_rs::ntlm::{create_ntlm_response, ntlmv2_response};

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16).unwrap();
            let low = (pair[1] as char).to_digit(16).unwrap();
            ((high << 4) | low) as u8
        })
        .collect()
}

fn node_type_three(domain: &str) -> Vec<u8> {
    create_ntlm_response(
        domain, "username", "password", &[0xbb; 8], &[0xaa; 4], &[3; 8], 0,
    )
    .unwrap()
}

#[test]
fn type_three_matches_node_for_domain_matrix_byte_for_byte() {
    let expected_without_domain = decode_hex(
        "4e544c4d53535000030000001800180050000000100010006800000000000000400000001000100040000000000000004000000000000000400000000182080075007300650072006e0061006d006500687f723233c2a509e35b5679311efcf60303030303030303e404be6ab7e5013e4c7bd0171b51ca87010100000000000000803ed5deb19d01030303030303030300000000aaaaaaaa00000000",
    );
    let expected_with_domain = decode_hex(
        "4e544c4d5353500003000000180018005c00000010001000740000000c000c0040000000100010004c000000000000004000000000000000400000000182080064006f006d00610069006e0075007300650072006e0061006d006500f34c7d931408c5fa86a2a67913e620140303030303030303e4182e1cd5053c8736deb888b6647dcb010100000000000000803ed5deb19d01030303030303030300000000aaaaaaaa00000000",
    );
    let expected_upper_domain = decode_hex(
        "4e544c4d5353500003000000180018005c00000010001000740000000c000c0040000000100010004c000000000000004000000000000000400000000182080044004f004d00410049004e0075007300650072006e0061006d006500f34c7d931408c5fa86a2a67913e620140303030303030303e4182e1cd5053c8736deb888b6647dcb010100000000000000803ed5deb19d01030303030303030300000000aaaaaaaa00000000",
    );

    let cases = [
        ("domain absent", "", expected_without_domain.clone()),
        ("domain empty", "", expected_without_domain),
        ("domain informed", "domain", expected_with_domain),
        ("domain informed uppercase", "DOMAIN", expected_upper_domain),
    ];
    for (label, domain, expected) in cases {
        assert_eq!(node_type_three(domain), expected, "{label}");
    }
}

#[test]
fn type_three_preserves_offsets_flags_timestamp_and_nonce() {
    let data = node_type_three("domain");
    assert_eq!(&data[0..8], b"NTLMSSP\0");
    assert_eq!(u32::from_le_bytes(data[8..12].try_into().unwrap()), 3);
    assert_eq!(u16::from_le_bytes(data[12..14].try_into().unwrap()), 24);
    assert_eq!(u32::from_le_bytes(data[16..20].try_into().unwrap()), 92);
    assert_eq!(u16::from_le_bytes(data[20..22].try_into().unwrap()), 16);
    assert_eq!(u32::from_le_bytes(data[24..28].try_into().unwrap()), 116);
    assert_eq!(u16::from_le_bytes(data[28..30].try_into().unwrap()), 12);
    assert_eq!(u32::from_le_bytes(data[32..36].try_into().unwrap()), 64);
    assert_eq!(u16::from_le_bytes(data[36..38].try_into().unwrap()), 16);
    assert_eq!(u32::from_le_bytes(data[40..44].try_into().unwrap()), 76);
    assert_eq!(u16::from_le_bytes(data[60..62].try_into().unwrap()), 0x8201);
    assert_eq!(u16::from_le_bytes(data[62..64].try_into().unwrap()), 8);
    assert_eq!(
        &data[92..108],
        &[
            0xf3, 0x4c, 0x7d, 0x93, 0x14, 0x08, 0xc5, 0xfa, 0x86, 0xa2, 0xa6, 0x79, 0x13, 0xe6,
            0x20, 0x14
        ]
    );
    assert_eq!(&data[132..136], &[1, 1, 0, 0]);
    assert_eq!(&data[136..140], &[0; 4]);
    assert_eq!(
        &data[140..148],
        &[0x00, 0x80, 0x3e, 0xd5, 0xde, 0xb1, 0x9d, 0x01]
    );
    assert_eq!(&data[148..156], &[3; 8]);
}

#[test]
fn malformed_nonce_lengths_return_errors_without_panicking() {
    assert!(
        create_ntlm_response(
            "domain", "username", "password", &[0xbb; 7], &[0xaa; 4], &[3; 8], 0,
        )
        .is_err()
    );
    assert!(
        ntlmv2_response(
            "domain", "username", "password", &[0xbb; 8], &[0xaa; 4], &[3; 7], 0,
        )
        .is_err()
    );
    assert!(
        ntlmv2_response(
            "domain", "username", "password", &[0xbb; 7], &[0xaa; 4], &[3; 8], 0,
        )
        .is_err()
    );
}
