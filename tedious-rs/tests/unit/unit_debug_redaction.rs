use std::sync::{Arc, Mutex};

use tedious_rs::connection::RoutingData;
use tedious_rs::debug::DebugOptions;
use tedious_rs::login7_payload::{FedAuth, Login7Options, Login7Payload};
use tedious_rs::message::Message;
use tedious_rs::message::MessageStreamEvent;
use tedious_rs::metadata_parser::read_metadata;
use tedious_rs::ntlm::parse_challenge;
use tedious_rs::ntlm_payload::{NtlmResponseOptions, NtlmResponsePayload};
use tedious_rs::packet::packet_type;
use tedious_rs::token::parser::RoutingChange;
use tedious_rs::token::parser::{EnvChangeKind, EnvChangeToken, MessageToken, ReturnValueToken};
use tedious_rs::token::parser::{FeatureExtAckToken, SspiToken};
use tedious_rs::token::rows::{
    ColumnEncryptionMetadata, ColumnMetadata, Row, RowColumn, TableName,
};
use tedious_rs::token::stream::{StreamToken, TokenStreamParser};
use tedious_rs::transport::{IncomingMessageChunk, MessageIo};
use tedious_rs::value_codec::{EncodedParameter, Value};

fn challenge() -> tedious_rs::ntlm::NtlmChallenge {
    let mut data = vec![0u8; 100];
    data[0..8].copy_from_slice(b"NTLMSSP\0");
    data[8..12].copy_from_slice(&2u32.to_le_bytes());
    data[12..14].copy_from_slice(&12u16.to_le_bytes());
    data[14..16].copy_from_slice(&12u16.to_le_bytes());
    data[16..20].copy_from_slice(&80u32.to_le_bytes());
    data[24..32].copy_from_slice(&[0xA1; 8]);
    data[40..42].copy_from_slice(&4u16.to_le_bytes());
    data[42..44].copy_from_slice(&4u16.to_le_bytes());
    data[44..48].copy_from_slice(&96u32.to_le_bytes());
    parse_challenge(&data).unwrap()
}

#[test]
fn sensitive_ntlm_debug_values_are_redacted() {
    let options = NtlmResponseOptions {
        domain: "domain".to_owned(),
        user_name: "user".to_owned(),
        password: "password-secret".to_owned(),
        challenge: challenge(),
        client_nonce: Some([1; 8]),
        unix_millis: Some(0),
    };
    let options_debug = format!("{options:?}");
    let payload = NtlmResponsePayload::new(options).unwrap();
    let payload_debug = format!("{payload:?}");

    assert!(!options_debug.contains("password-secret"));
    assert!(!payload_debug.contains(&format!("{:02x}", payload.data[0])));
    assert!(payload_debug.contains("REDACTED"));
}

#[test]
fn authentication_struct_debug_values_are_redacted() {
    let challenge = challenge();
    let challenge_debug = format!("{challenge:?}");
    assert!(!challenge_debug.contains("a1a1a1a1"));
    assert!(challenge_debug.contains("REDACTED"));

    let sspi = SspiToken {
        ntlm_packet: challenge,
        buffer: vec![0xB2; 12],
    };
    let sspi_debug = format!("{sspi:?}");
    assert!(!sspi_debug.contains("b2b2b2b2"));
    assert!(sspi_debug.contains("[REDACTED; 12 bytes]"));

    let feature = FeatureExtAckToken {
        fed_auth: Some(vec![0xC3; 4]),
        utf8_support: Some(true),
    };
    let feature_debug = format!("{feature:?}");
    assert!(!feature_debug.contains("c3c3c3c3"));
    assert!(feature_debug.contains("[REDACTED; 4 bytes]"));
}

#[test]
fn fed_auth_and_login_sspi_debug_values_are_redacted() {
    let fed_auth = FedAuth::SecurityToken {
        echo: true,
        token: "access-token-secret".to_owned(),
    };
    let fed_auth_debug = format!("{fed_auth:?}");
    assert!(!fed_auth_debug.contains("access-token-secret"));
    assert!(fed_auth_debug.contains("[REDACTED; 19 bytes]"));

    let mut payload = Login7Payload::new(Login7Options {
        tds_version: 0x74000004,
        packet_size: 4096,
        client_prog_ver: 1,
        client_pid: 2,
        connection_id: 3,
        client_time_zone: 0,
        client_lcid: 0x409,
    });
    payload.sspi = Some(vec![0xD4; 6]);
    let payload_debug = format!("{payload:?}");
    assert!(!payload_debug.contains("d4d4d4d4"));
    assert!(payload_debug.contains("[REDACTED; 6 bytes]"));
}

#[test]
fn externally_debugged_transport_values_do_not_include_message_bytes() {
    let chunk = IncomingMessageChunk {
        message_type: packet_type::SQL_BATCH,
        reset_connection: false,
        ignore: false,
        data: b"select secret_value".to_vec(),
        end_of_message: true,
    };
    let routing = RoutingData {
        protocol: 1,
        server: "server".to_owned(),
        port: 1433,
        instance: None,
        old_value: b"routing-secret".to_vec(),
    };

    assert!(!format!("{chunk:?}").contains("secret_value"));
    assert!(!format!("{routing:?}").contains("routing-secret"));
}

#[test]
fn message_value_parameter_and_stream_debug_values_are_redacted() {
    let message = Message {
        message_type: packet_type::SQL_BATCH,
        reset_connection: false,
        ignore: false,
        data: b"select secret_value".to_vec(),
    };
    let event = MessageStreamEvent::Data {
        message_type: packet_type::SQL_BATCH,
        data: b"select secret_value".to_vec(),
    };
    let parameter = EncodedParameter {
        type_info: vec![1, 2],
        length: vec![3],
        data: b"secret_value".to_vec(),
    };
    let value = Value::Text("secret_value".to_owned());

    assert!(!format!("{message:?}").contains("secret_value"));
    assert!(!format!("{event:?}").contains("secret_value"));
    assert!(!format!("{parameter:?}").contains("secret_value"));
    assert!(!format!("{value:?}").contains("secret_value"));
    assert!(format!("{message:?}").contains("19 bytes"));
    assert!(format!("{parameter:?}").contains("12 bytes"));

    let mut parser = TokenStreamParser::new("7_4", false);
    let metadata = [0x81, 1, 0, 0, 0, 0, 0, 0, 0, 0x38, 2, b'I', 0, b'd', 0];
    parser.feed(&metadata).unwrap();
    let tokens = parser.feed(&[0xD1, 7, 0, 0, 0]).unwrap();
    let token = &tokens[0];
    assert!(matches!(token, StreamToken::Row(_)));
    assert!(!format!("{token:?}").contains("secret"));
    assert!(format!("{token:?}").contains("1 columns"));
}

#[test]
fn token_debug_values_and_metadata_are_redacted() {
    let metadata_value = read_metadata(&[0, 0, 0, 0, 0, 0, 0x38], 0, "7_4")
        .unwrap()
        .value;
    let message = MessageToken {
        token: 1,
        number: 2,
        state: 3,
        class: 4,
        message: "server-message-secret".to_owned(),
        server_name: "server-secret".to_owned(),
        proc_name: "procedure-secret".to_owned(),
        line_number: 5,
    };
    let return_value = ReturnValueToken {
        ordinal: 1,
        name: "parameter-secret".to_owned(),
        status: 0,
        metadata: metadata_value.clone(),
        value: Value::Text("return-secret".to_owned()),
    };
    let env_change = EnvChangeToken {
        change_type: 1,
        kind: EnvChangeKind::Text {
            new_value: "new-secret".to_owned(),
            old_value: "old-secret".to_owned(),
        },
    };
    let routing = RoutingChange {
        protocol: 1,
        port: 1433,
        server: "server-secret".to_owned(),
        old_value: b"route-secret".to_vec(),
    };
    let metadata = ColumnMetadata {
        metadata: metadata_value,
        col_name: "column-secret".to_owned(),
        table_name: Some(TableName::Legacy("table-secret".to_owned())),
        crypto_metadata: Some(ColumnEncryptionMetadata {
            ordinal: 0,
            cipher_algorithm_id: 1,
            encryption_type:
                tedious_rs::always_encrypted::types::SqlServerEncryptionType::Deterministic,
            normalization_rule_version: b"metadata-secret".to_vec(),
        }),
    };
    let row = Row {
        columns: vec![RowColumn {
            value: Value::Text("row-secret".to_owned()),
            metadata,
        }],
    };

    for rendered in [
        format!("{message:?}"),
        format!("{return_value:?}"),
        format!("{env_change:?}"),
        format!("{routing:?}"),
        format!("{row:?}"),
    ] {
        assert!(!rendered.contains("secret"));
        assert!(rendered.contains("REDACTED"));
    }
}

#[tokio::test]
async fn packet_debug_event_redacts_packet_data() {
    let (client, _server) = tokio::io::duplex(128);
    let mut io = MessageIo::new(client, 64).unwrap();
    let messages = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&messages);
    io.set_debug(
        DebugOptions {
            data: true,
            packet: true,
            payload: false,
            token: false,
        },
        Arc::new(move |message| captured.lock().unwrap().push(message)),
    );
    let mut message = Message::new(packet_type::SQL_BATCH);
    message.data = b"select secret_value".to_vec();
    io.send_message(&message).await.unwrap();

    let output = messages.lock().unwrap().join("\n");
    assert!(!output.contains("secret_value"));
    assert!(output.contains("REDACTED PACKET DATA"));
    assert!(output.contains("SQL_BATCH"));
}
