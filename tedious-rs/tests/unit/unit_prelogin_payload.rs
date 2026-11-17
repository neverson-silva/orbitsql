use tedious_rs::prelogin_payload::{
    PreloginOptions, PreloginPayload, PreloginVersion, TOKEN_TRACE_ID,
};

#[test]
fn creates_and_round_trips_prelogin_options() {
    let payload = PreloginPayload::new(PreloginOptions {
        encrypt: true,
        version: PreloginVersion {
            major: 15,
            minor: 0,
            build: 4312,
            subbuild: 1,
        },
    })
    .unwrap();
    assert_eq!(payload.encryption, 1);
    assert_eq!(payload.version.build, 4312);
    assert_eq!(payload.trace_id.len(), 36);
    let parsed = PreloginPayload::from_bytes(payload.data.clone()).unwrap();
    assert_eq!(parsed.data, payload.data);
    assert_eq!(parsed.encryption_string, Some("ON"));
    assert_eq!(parsed.mars_string, Some("OFF"));
    assert!(
        parsed
            .to_string("  ")
            .starts_with("  PreLogin - version:15.0.4312.1")
    );
}

#[test]
fn parses_all_wire_fields_and_unknown_tokens_without_reordering() {
    let values = [
        (0, vec![1, 2, 0x12, 0x34, 0x56, 0x78]),
        (1, vec![3]),
        (2, vec![7]),
        (3, vec![0x11, 0x22, 0x33, 0x44]),
        (4, vec![1]),
        (6, vec![1]),
        (9, Vec::new()),
    ];
    let mut data = vec![0; values.len() * 5 + 1];
    let mut offset = data.len();
    for (index, (token, value)) in values.iter().enumerate() {
        let header = index * 5;
        data[header] = *token;
        data[header + 1..header + 3].copy_from_slice(&(offset as u16).to_be_bytes());
        data[header + 3..header + 5].copy_from_slice(&(value.len() as u16).to_be_bytes());
        data.extend_from_slice(value);
        offset += value.len();
    }
    data[values.len() * 5] = 0xFF;
    let parsed = PreloginPayload::from_bytes(data).unwrap();
    assert_eq!(parsed.version.major, 1);
    assert_eq!(parsed.encryption_string, Some("REQ"));
    assert_eq!(parsed.instance, 7);
    assert_eq!(parsed.thread_id, 0x11223344);
    assert_eq!(parsed.mars_string, Some("ON"));
    assert_eq!(parsed.fed_auth_required, 1);
}

#[test]
fn rejects_truncated_directory_and_option_data() {
    assert!(PreloginPayload::from_bytes(vec![0, 0, 1]).is_err());
    assert!(PreloginPayload::from_bytes(vec![0, 0, 10, 0, 6, 0xFF]).is_err());
}

#[test]
fn limits_trace_id_to_the_node_wire_width() {
    let trace = vec![0xAB; 40];
    let mut data = vec![TOKEN_TRACE_ID, 0, 6, 0, 40, 0xFF];
    data.extend_from_slice(&trace);
    let parsed = PreloginPayload::from_bytes(data).unwrap();
    assert_eq!(parsed.trace_id, vec![0xAB; 36]);
}
