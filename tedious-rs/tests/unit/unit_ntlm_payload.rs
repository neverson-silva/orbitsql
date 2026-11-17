use tedious_rs::ntlm::parse_challenge;
use tedious_rs::ntlm_payload::{NtlmResponseOptions, NtlmResponsePayload};

fn challenge() -> tedious_rs::ntlm::NtlmChallenge {
    let domain_offset = 80u32;
    let target_offset = 96u32;
    let mut data = vec![0u8; target_offset as usize + 4];
    data[0..8].copy_from_slice(b"NTLMSSP\0");
    data[8..12].copy_from_slice(&2u32.to_le_bytes());
    data[12..14].copy_from_slice(&12u16.to_le_bytes());
    data[14..16].copy_from_slice(&12u16.to_le_bytes());
    data[16..20].copy_from_slice(&domain_offset.to_le_bytes());
    data[24..32].copy_from_slice(&[0xA1; 8]);
    data[40..42].copy_from_slice(&4u16.to_le_bytes());
    data[42..44].copy_from_slice(&4u16.to_le_bytes());
    data[44..48].copy_from_slice(&target_offset.to_le_bytes());
    data[domain_offset as usize..domain_offset as usize + 12].copy_from_slice(&[0; 12]);
    parse_challenge(&data).unwrap()
}

#[test]
fn builds_type_three_payload_with_deterministic_injected_nonce_and_time() {
    let payload = NtlmResponsePayload::new(NtlmResponseOptions {
        domain: "domain".to_owned(),
        user_name: "username".to_owned(),
        password: "password".to_owned(),
        challenge: challenge(),
        client_nonce: Some([1, 2, 3, 4, 5, 6, 7, 8]),
        unix_millis: Some(0),
    })
    .unwrap();
    assert_eq!(&payload.data[0..8], b"NTLMSSP\0");
    assert_eq!(
        u32::from_le_bytes(payload.data[8..12].try_into().unwrap()),
        3
    );
    assert_eq!(payload.to_string("  "), "  NTLM Auth");
}
