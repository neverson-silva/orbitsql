use tedious_rs::ntlm::{
    create_ntlm_request, create_ntlm_response, create_timestamp, lmv2_response, nt_hash,
    ntlmv2_response, ntv2_hash, parse_challenge,
};

#[test]
fn creates_type_one_payload_with_optional_workstation_flag() {
    let with_workstation = create_ntlm_request("domain", Some("workstation"));
    let without_workstation = create_ntlm_request("domain", None);
    assert_eq!(&with_workstation[0..8], b"NTLMSSP\0");
    assert_eq!(
        u32::from_le_bytes(with_workstation[8..12].try_into().unwrap()),
        1
    );
    assert_eq!(&with_workstation[40..51], b"WORKSTATION");
    assert_eq!(&with_workstation[51..57], b"DOMAIN");
    let with_flags = u32::from_le_bytes(with_workstation[12..16].try_into().unwrap());
    let without_flags = u32::from_le_bytes(without_workstation[12..16].try_into().unwrap());
    assert_ne!(with_flags & 0x2000, 0);
    assert_eq!(without_flags & 0x2000, 0);
}

#[test]
fn matches_md4_nt_hash_and_ntlm_timestamp_vectors() {
    assert_eq!(
        hex(&nt_hash("Password")),
        "a4f49c406510bdcab6824ee7c30fd852"
    );
    assert_eq!(create_timestamp(0), 0x019db1ded53e8000u64.to_le_bytes());
}

#[test]
fn computes_ntlmv2_hash_and_lmv2_response_deterministically() {
    let hash = ntv2_hash("DOMAIN", "User", "Password").unwrap();
    assert_eq!(hex(&hash), "f38efea48ada6afaa95ae44669e5634b");
    let response = lmv2_response(
        "DOMAIN",
        "User",
        "Password",
        &[1, 2, 3, 4, 5, 6, 7, 8],
        &[9, 10, 11, 12, 13, 14, 15, 16],
    )
    .unwrap();
    assert_eq!(response.len(), 24);
    assert_eq!(&response[16..], &[9, 10, 11, 12, 13, 14, 15, 16]);
    let nt_response = ntlmv2_response(
        "DOMAIN",
        "User",
        "Password",
        &[1, 2, 3, 4, 5, 6, 7, 8],
        &[0x02, 0, 0, 0],
        &[9, 10, 11, 12, 13, 14, 15, 16],
        0,
    )
    .unwrap();
    assert_eq!(nt_response.len(), 16);
}

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn parses_type_two_challenge_security_buffers() {
    let domain_offset = 80u32;
    let target_offset = 96u32;
    let mut data = vec![0u8; target_offset as usize + 4];
    data[0..8].copy_from_slice(b"NTLMSSP\0");
    data[8..12].copy_from_slice(&2u32.to_le_bytes());
    data[12..14].copy_from_slice(&12u16.to_le_bytes());
    data[14..16].copy_from_slice(&12u16.to_le_bytes());
    data[16..20].copy_from_slice(&domain_offset.to_le_bytes());
    data[20..24].copy_from_slice(&11256099u32.to_le_bytes());
    data[24..32].copy_from_slice(&[0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0xA7, 0xB8]);
    data[40..42].copy_from_slice(&4u16.to_le_bytes());
    data[42..44].copy_from_slice(&4u16.to_le_bytes());
    data[44..48].copy_from_slice(&target_offset.to_le_bytes());
    data[48..56].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    data[domain_offset as usize..domain_offset as usize + 12].copy_from_slice(
        &"domain"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    data[target_offset as usize..target_offset as usize + 4].copy_from_slice(&[0, 0, 0, 0]);
    let challenge = parse_challenge(&data).unwrap();
    assert_eq!(challenge.message_type, 2);
    assert_eq!(
        challenge.nonce,
        [0xA1, 0xB2, 0xC3, 0xD4, 0xE5, 0xF6, 0xA7, 0xB8]
    );
    assert_eq!(
        challenge.domain,
        "domain"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(challenge.target, vec![0, 0, 0, 0]);
}

#[test]
fn rejects_challenge_with_invalid_signature_or_declared_range() {
    let mut data = vec![0u8; 56];
    data[8..12].copy_from_slice(&2u32.to_le_bytes());
    assert!(parse_challenge(&data).is_err());
    data[0..8].copy_from_slice(b"NTLMSSP\0");
    data[12..14].copy_from_slice(&1u16.to_le_bytes());
    data[16..20].copy_from_slice(&100u32.to_le_bytes());
    assert!(parse_challenge(&data).is_err());
}

#[test]
fn creates_type_three_response_with_stable_offsets() {
    let response = create_ntlm_response(
        "domain",
        "username",
        "password",
        &[0xBB; 8],
        &[0xAA; 4],
        &[1, 2, 3, 4, 5, 6, 7, 8],
        0,
    )
    .unwrap();
    assert_eq!(&response[0..8], b"NTLMSSP\0");
    assert_eq!(u32::from_le_bytes(response[8..12].try_into().unwrap()), 3);
    assert_eq!(
        &response[64..76],
        "domain"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        &response[76..92],
        "username"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(response.len(), 168);
}
