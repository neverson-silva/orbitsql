use tedious_rs::login7_payload::{FedAuth, Login7Options, Login7Payload};

fn payload() -> Login7Payload {
    let mut payload = Login7Payload::new(Login7Options {
        tds_version: 0x74000004,
        packet_size: 4096,
        client_prog_ver: 1,
        client_pid: 2,
        connection_id: 3,
        client_time_zone: -180,
        client_lcid: 0x409,
    });
    payload.hostname = Some("host".to_owned());
    payload.user_name = Some("user".to_owned());
    payload.password = Some("pass".to_owned());
    payload.database = Some("db".to_owned());
    payload.read_only_intent = true;
    payload.client_id = Some(vec![1, 2, 3, 4, 5, 6]);
    payload
}

#[test]
fn debug_redacts_login_credentials_and_tokens() {
    let mut value = payload();
    value.fed_auth = Some(FedAuth::SecurityToken {
        echo: true,
        token: "token-value".to_owned(),
    });
    let rendered = format!("{value:?}");
    assert!(!rendered.contains("\"pass\""));
    assert!(!rendered.contains("token-value"));
    assert!(rendered.contains("[REDACTED]"));
}

#[test]
fn serializes_login7_fixed_fields_and_scrambled_password() {
    let data = payload().to_buffer().unwrap();
    assert_eq!(
        u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize,
        data.len()
    );
    assert_eq!(&data[4..8], &0x74000004u32.to_le_bytes());
    assert_eq!(data[25], 0);
    assert_eq!(data[26] & 0x20, 0x20);
    assert_eq!(data[24] & 0xE0, 0xA0);
    let password_offset = u16::from_le_bytes(data[44..46].try_into().unwrap()) as usize;
    let password_units = u16::from_le_bytes(data[46..48].try_into().unwrap()) as usize;
    assert_eq!(
        &data[password_offset..password_offset + password_units * 2],
        &[0xA2, 0xA5, 0xB3, 0xA5, 0x92, 0xA5, 0x92, 0xA5]
    );
}

#[test]
fn appends_feature_extensions_after_variable_data() {
    let mut value = payload();
    value.fed_auth = Some(FedAuth::Adal {
        echo: true,
        integrated: false,
    });
    let data = value.to_buffer().unwrap();
    let extension_slot = u16::from_le_bytes(data[56..58].try_into().unwrap()) as usize;
    let extension_pointer =
        u32::from_le_bytes(data[extension_slot..extension_slot + 4].try_into().unwrap()) as usize;
    assert!(extension_pointer >= 98);
    assert_eq!(data[extension_pointer], 2);
    assert_eq!(data[data.len() - 1], 0xFF);
}

#[test]
fn supports_security_token_feature_extension() {
    let mut value = payload();
    value.fed_auth = Some(FedAuth::SecurityToken {
        echo: false,
        token: "token".to_owned(),
    });
    let data = value.to_buffer().unwrap();
    assert!(data.windows(1).any(|pair| pair == [2]));
}

#[test]
fn uses_node_adal_library_and_echo_values() {
    let mut value = payload();
    value.fed_auth = Some(FedAuth::Adal {
        echo: true,
        integrated: true,
    });
    let data = value.to_buffer().unwrap();
    let extension_slot = u16::from_le_bytes(data[56..58].try_into().unwrap()) as usize;
    let extension_pointer =
        u32::from_le_bytes(data[extension_slot..extension_slot + 4].try_into().unwrap()) as usize;
    assert_eq!(
        &data[extension_pointer..extension_pointer + 7],
        &[2, 2, 0, 0, 0, 5, 2]
    );
}

#[test]
fn preserves_security_token_feature_length_beyond_one_byte() {
    let mut value = payload();
    value.fed_auth = Some(FedAuth::SecurityToken {
        echo: false,
        token: "A".repeat(130),
    });
    let data = value.to_buffer().unwrap();
    let extension_slot = u16::from_le_bytes(data[56..58].try_into().unwrap()) as usize;
    let extension_pointer =
        u32::from_le_bytes(data[extension_slot..extension_slot + 4].try_into().unwrap()) as usize;
    assert_eq!(
        &data[extension_pointer..extension_pointer + 5],
        &[2, 9, 1, 0, 0]
    );
}

#[test]
fn rejects_login7_string_lengths_that_would_truncate_u16_fields() {
    let mut value = payload();
    value.hostname = Some("A".repeat(usize::from(u16::MAX) + 1));
    assert!(value.to_buffer().is_err());
}

#[test]
fn rejects_login7_client_id_longer_than_the_fixed_wire_field() {
    let mut value = payload();
    value.client_id = Some(vec![0; 7]);
    assert!(value.to_buffer().is_err());
}

#[test]
fn rejects_login7_offsets_that_would_overflow_u16() {
    let mut value = payload();
    value.hostname = Some("A".repeat(32_700));
    value.user_name = Some("B".repeat(32_700));
    assert!(value.to_buffer().is_err());
}

#[test]
fn preserves_long_sspi_length_without_truncating_the_long_length_field() {
    let mut value = payload();
    value.sspi = Some(vec![0xA5; usize::from(u16::MAX) + 1]);
    let data = value.to_buffer().unwrap();
    assert_eq!(u16::from_le_bytes(data[80..82].try_into().unwrap()), 65_535);
    assert_eq!(u32::from_le_bytes(data[90..94].try_into().unwrap()), 65_536);
}
