use std::path::Path;
use std::process::Command;

use tedious_rs::login7_payload::{Login7Options, Login7Payload};
use tedious_rs::prelogin_payload::{PreloginOptions, PreloginPayload, PreloginVersion};

fn node_payloads() -> serde_json::Value {
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/compatibility/node_payload_parity.js");
    let output = Command::new("node")
        .arg(script)
        .output()
        .expect("Node must be available for payload parity");
    assert!(
        output.status.success(),
        "Node payload observer failed with status {}",
        output.status
    );
    serde_json::from_slice(&output.stdout).expect("Node payload observer must emit JSON")
}

fn normalize_prelogin(mut data: Vec<u8>) -> Vec<u8> {
    let mut offset = 0;
    while data[offset] != 0xff {
        let token = data[offset];
        let start = u16::from_be_bytes([data[offset + 1], data[offset + 2]]) as usize;
        let length = u16::from_be_bytes([data[offset + 3], data[offset + 4]]) as usize;
        if token == tedious_rs::prelogin_payload::TOKEN_TRACE_ID {
            data[start..start + length].fill(0);
        }
        offset += 5;
    }
    data
}

fn encode_hex(data: &[u8]) -> String {
    data.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn node_and_rust_prelogin_payloads_match_byte_for_byte() {
    let rust = PreloginPayload::new(PreloginOptions {
        encrypt: true,
        version: PreloginVersion {
            major: 1,
            minor: 2,
            build: 3,
            subbuild: 4,
        },
    })
    .unwrap();
    let node = node_payloads()["prelogin"].as_str().unwrap().to_owned();
    assert_eq!(encode_hex(&normalize_prelogin(rust.data)), node);
}

#[test]
fn node_and_rust_login7_payloads_match_byte_for_byte() {
    let mut rust = Login7Payload::new(Login7Options {
        tds_version: 0x74000004,
        packet_size: 4096,
        client_prog_ver: 0,
        client_pid: 1234,
        connection_id: 0,
        client_time_zone: 0,
        client_lcid: 0x409,
    });
    rust.user_name = Some("sa".to_owned());
    rust.password = Some("password".to_owned());
    rust.server_name = Some("localhost".to_owned());
    rust.app_name = Some("Tedious".to_owned());
    rust.hostname = Some("host".to_owned());
    rust.library_name = Some("Tedious".to_owned());
    rust.database = Some("tedious_rs".to_owned());
    rust.client_id = Some(vec![1, 2, 3, 4, 5, 6]);
    let node = node_payloads()["login7"].as_str().unwrap().to_owned();
    assert_eq!(encode_hex(&rust.to_buffer().unwrap()), node);
}

fn base_login7() -> Login7Payload {
    Login7Payload::new(Login7Options {
        tds_version: 0x74000004,
        packet_size: 4096,
        client_prog_ver: 0,
        client_pid: 1234,
        connection_id: 0,
        client_time_zone: 0,
        client_lcid: 0x409,
    })
}

#[test]
fn node_and_rust_ntlm_login7_payloads_match_byte_for_byte() {
    let mut rust = base_login7();
    rust.server_name = Some("localhost".to_owned());
    rust.sspi = Some(vec![1, 2, 3, 4]);
    let node = node_payloads()["login7Ntlm"].as_str().unwrap().to_owned();
    assert_eq!(encode_hex(&rust.to_buffer().unwrap()), node);
}

#[test]
fn node_and_rust_fedauth_login7_payloads_match_byte_for_byte() {
    let mut rust = base_login7();
    rust.server_name = Some("localhost".to_owned());
    rust.fed_auth = Some(tedious_rs::login7_payload::FedAuth::Adal {
        echo: true,
        integrated: true,
    });
    let node = node_payloads()["login7FedAuth"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(encode_hex(&rust.to_buffer().unwrap()), node);
}
