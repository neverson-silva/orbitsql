use std::sync::{Arc, Mutex};

use serde_json::Value;
use tedious_rs::errors::RequestError;
use tedious_rs::login7_payload::{Login7Options, Login7Payload};
use tedious_rs::payloads::{
    BulkLoadColumn, BulkLoadPayload, BulkLoadRow, RpcParameter, RpcRequestPayload, SqlBatchPayload,
    TvpColumnDefinition, TvpPayload, TvpRow,
};
use tedious_rs::prelogin_payload::PreloginPayload;
use tedious_rs::request::{Request, RequestEvent};
use tedious_rs::value_codec::{EncodedParameter, Value as CodecValue};
use tedious_rs::{ParameterOptions, SqlParameterType};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../fixtures/compatibility/request_observations.json"
    ))
    .expect("compatibility fixture must be valid JSON")
}

fn scenario(name: &str) -> Value {
    fixture()["scenarios"]
        .as_array()
        .expect("compatibility scenarios must be an array")
        .iter()
        .find(|scenario| scenario["name"] == name)
        .cloned()
        .expect("compatibility scenario must exist")
}

fn expected_values(scenario: &Value, field: &str) -> Vec<String> {
    scenario[field]
        .as_array()
        .expect("compatibility expected observations must be an array")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("observation must be a string")
                .to_owned()
        })
        .collect()
}

fn expected_dimension(name: &str, dimension: &str, side: &str) -> Option<Vec<String>> {
    scenario(name)[dimension][side].as_array().map(|values| {
        values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("observation must be a string")
                    .to_owned()
            })
            .collect()
    })
}

fn assert_dimension(name: &str, dimension: &str, observed: &[String]) {
    let scenario = scenario(name);
    let expected = expected_dimension(name, dimension, "rust");
    assert_eq!(
        Some(observed.to_vec()),
        expected,
        "Rust {dimension} differs for compatibility scenario {name}"
    );
    let node = scenario[dimension]["node"].as_array();
    if let Some(node) = node {
        let expected = node
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("Node observation must be a string")
                    .to_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            observed, expected,
            "Rust and Node {dimension} differ for {name}"
        );
    } else {
        assert!(scenario[dimension]["gap"].as_str().is_some());
    }
}

fn assert_expected(name: &str, observed: &[String]) {
    let scenario = scenario(name);
    assert_eq!(
        observed,
        expected_values(&scenario, "rust_expected"),
        "Rust observation differs for compatibility scenario {name}"
    );
    let node_expected = scenario["node_expected"].as_array();
    if let Some(node_expected) = node_expected {
        let node_expected = node_expected
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("Node observation must be a string")
                    .to_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            observed, node_expected,
            "Rust and Node observations differ for {name}"
        );
        assert!(scenario["gaps"].as_array().is_some_and(Vec::is_empty));
    } else {
        assert!(!scenario["gaps"].as_array().is_some_and(Vec::is_empty));
    }
}

fn event_name(event: &RequestEvent) -> Option<String> {
    match event {
        RequestEvent::Error(error) => Some(format!(
            "event:error:{}",
            error.code.as_deref().unwrap_or("")
        )),
        RequestEvent::RequestCompleted => Some("event:requestCompleted".to_owned()),
        _ => None,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn node_bytes(name: &str) -> Option<String> {
    let output = std::process::Command::new("node")
        .arg("tests/compatibility/node_bytes_observer.js")
        .arg(name)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}

fn node_request_observations(mode: &str) -> Option<Vec<String>> {
    let output = std::process::Command::new("node")
        .arg("tests/compatibility/node_request_observer.js")
        .arg(mode)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

fn state_name(state: tedious_rs::request::RequestState) -> String {
    format!("state:{state:?}")
}

fn fixture_hex(source: &str) -> String {
    let value: Value = serde_json::from_str(source).expect("fixture must be valid JSON");
    value["expected_hex"]
        .as_str()
        .expect("fixture must contain expected_hex")
        .to_owned()
}

fn int_parameter(value: i64) -> EncodedParameter {
    SqlParameterType::Int
        .encode(&CodecValue::Integer(value), &ParameterOptions::default())
        .unwrap()
}

fn compatibility_bytes(path: &str) -> Vec<u8> {
    match path {
        "packets/prelogin.json" => {
            let expected = decode_hex(&fixture_hex(include_str!(
                "../fixtures/packets/prelogin.json"
            )));
            PreloginPayload::from_bytes(expected)
                .expect("prelogin compatibility fixture must parse")
                .data
        }
        "packets/prelogin_node_request.json" => {
            let expected = decode_hex(&fixture_hex(include_str!(
                "../fixtures/packets/prelogin_node_request.json"
            )));
            PreloginPayload::from_bytes(expected)
                .expect("prelogin Node capture must parse")
                .data
        }
        "auth/login7_sql.json" => {
            let mut payload = Login7Payload::new(Login7Options {
                tds_version: 0x72090002,
                packet_size: 4096,
                client_prog_ver: 0x00000001,
                client_pid: 7,
                connection_id: 0,
                client_time_zone: 0,
                client_lcid: 0x0409,
            });
            payload.hostname = Some("compat".to_owned());
            payload.user_name = Some("sa".to_owned());
            payload.password = Some("x".to_owned());
            payload.app_name = Some("tedious-rs".to_owned());
            payload.server_name = Some("localhost".to_owned());
            payload.database = Some("tedious_rs".to_owned());
            payload
                .to_buffer()
                .expect("login7 compatibility fixture must encode")
        }
        "payloads/sql_batch.json" => SqlBatchPayload {
            sql_text: "SELECT 1".to_owned(),
            transaction_descriptor: vec![0; 8],
            tds_version: "7_4".to_owned(),
        }
        .buffers()
        .expect("SQL batch compatibility fixture must encode")
        .concat(),
        "payloads/rpc.json" => RpcRequestPayload {
            procedure: Some("sp_executesql".to_owned()),
            procedure_id: None,
            parameters: vec![RpcParameter {
                name: "p".to_owned(),
                output: false,
                encoded: int_parameter(7),
                encrypted_metadata: None,
            }],
            transaction_descriptor: vec![0; 8],
            tds_version: "7_4".to_owned(),
        }
        .buffers()
        .expect("RPC compatibility fixture must encode")
        .concat(),
        "payloads/tvp.json" => TvpPayload {
            database: "".to_owned(),
            schema: "dbo".to_owned(),
            type_name: "CompatType".to_owned(),
            columns: vec![TvpColumnDefinition {
                name: "id".to_owned(),
                type_info: vec![0x26, 4],
            }],
            rows: vec![TvpRow {
                values: vec![int_parameter(7)],
            }],
        }
        .buffers()
        .expect("TVP compatibility fixture must encode")
        .concat(),
        "payloads/bulk_load.json" => BulkLoadPayload {
            columns: vec![BulkLoadColumn {
                name: "id".to_owned(),
                table_name: "dbo.compat".to_owned(),
                type_info: vec![0x26, 4],
                nullable: false,
                legacy_lob: false,
            }],
            rows: vec![BulkLoadRow {
                values: vec![int_parameter(7)],
            }],
            tds_version: "7_4".to_owned(),
        }
        .data()
        .expect("bulk compatibility fixture must encode"),
        "types/int.json" => {
            let encoded = int_parameter(7);
            [encoded.type_info, encoded.length, encoded.data].concat()
        }
        path => panic!("unknown compatibility byte fixture: {path}"),
    }
}

#[test]
fn prelogin_fixture_round_trip_is_byte_exact() {
    let expected = decode_hex(&fixture_hex(include_str!(
        "../fixtures/packets/prelogin.json"
    )));
    let payload = PreloginPayload::from_bytes(expected.clone()).expect("fixture must parse");
    assert_eq!(payload.data, expected);
    assert_eq!(payload.version.major, 1);
    assert_eq!(payload.encryption, 1);
}

#[test]
fn login7_fixture_is_byte_exact() {
    let expected = decode_hex(&fixture_hex(include_str!(
        "../fixtures/auth/login7_sql.json"
    )));
    let mut payload = Login7Payload::new(Login7Options {
        tds_version: 0x72090002,
        packet_size: 4096,
        client_prog_ver: 0x00000001,
        client_pid: 7,
        connection_id: 0,
        client_time_zone: 0,
        client_lcid: 0x0409,
    });
    payload.hostname = Some("compat".to_owned());
    payload.user_name = Some("sa".to_owned());
    payload.password = Some("x".to_owned());
    payload.app_name = Some("tedious-rs".to_owned());
    payload.server_name = Some("localhost".to_owned());
    payload.database = Some("tedious_rs".to_owned());
    assert_eq!(payload.to_buffer().unwrap(), expected);
}

#[test]
fn sql_batch_fixture_is_byte_exact() {
    let expected = decode_hex(&fixture_hex(include_str!(
        "../fixtures/payloads/sql_batch.json"
    )));
    let payload = SqlBatchPayload {
        sql_text: "SELECT 1".to_owned(),
        transaction_descriptor: vec![0; 8],
        tds_version: "7_4".to_owned(),
    };
    let actual = payload.buffers().unwrap().concat();
    assert_eq!(actual, expected);
}

#[test]
fn rpc_fixture_is_byte_exact() {
    let expected = decode_hex(&fixture_hex(include_str!("../fixtures/payloads/rpc.json")));
    let payload = RpcRequestPayload {
        procedure: Some("sp_executesql".to_owned()),
        procedure_id: None,
        parameters: vec![RpcParameter {
            name: "p".to_owned(),
            output: false,
            encoded: int_parameter(7),
            encrypted_metadata: None,
        }],
        transaction_descriptor: vec![0; 8],
        tds_version: "7_4".to_owned(),
    };
    let actual = payload.buffers().unwrap().concat();
    assert_eq!(actual, expected);
}

#[test]
fn tvp_fixture_is_byte_exact() {
    let expected = decode_hex(&fixture_hex(include_str!("../fixtures/payloads/tvp.json")));
    let payload = TvpPayload {
        database: "".to_owned(),
        schema: "dbo".to_owned(),
        type_name: "CompatType".to_owned(),
        columns: vec![TvpColumnDefinition {
            name: "id".to_owned(),
            type_info: vec![0x26, 4],
        }],
        rows: vec![TvpRow {
            values: vec![int_parameter(7)],
        }],
    };
    let actual = payload.buffers().unwrap().concat();
    assert_eq!(actual, expected);
}

#[test]
fn bulk_load_fixture_is_byte_exact() {
    let expected = decode_hex(&fixture_hex(include_str!(
        "../fixtures/payloads/bulk_load.json"
    )));
    let payload = BulkLoadPayload {
        columns: vec![BulkLoadColumn {
            name: "id".to_owned(),
            table_name: "dbo.compat".to_owned(),
            type_info: vec![0x26, 4],
            nullable: false,
            legacy_lob: false,
        }],
        rows: vec![BulkLoadRow {
            values: vec![int_parameter(7)],
        }],
        tds_version: "7_4".to_owned(),
    };
    assert_eq!(payload.data().unwrap(), expected);
}

#[test]
fn token_fixtures_are_byte_exact_and_parseable() {
    let value: Value = serde_json::from_str(include_str!("../fixtures/tokens/basic.json")).unwrap();
    for item in value["cases"].as_array().unwrap() {
        let bytes = decode_hex(item["hex"].as_str().unwrap());
        let parsed = tedious_rs::token::parser::parse_token(&bytes, 0, "7_4");
        let parsed = parsed.expect("token fixture must parse");
        match item["name"].as_str().unwrap() {
            "done_success" => match parsed.value {
                tedious_rs::token::parser::Token::Done(token) => {
                    assert_eq!(token.token, 0xfd);
                    assert_eq!(token.row_count, None);
                }
                _ => panic!("DONE fixture parsed as another token"),
            },
            "return_status_zero" => match parsed.value {
                tedious_rs::token::parser::Token::ReturnStatus(token) => {
                    assert_eq!(token.value, 0);
                }
                _ => panic!("RETURNSTATUS fixture parsed as another token"),
            },
            name => panic!("unknown token fixture: {name}"),
        }
        assert_eq!(hex(&bytes), item["hex"].as_str().unwrap());
    }
}

#[test]
fn compatibility_byte_matrix_matches_rust_and_marks_uncaptured_node_bytes() {
    let value: Value = serde_json::from_str(include_str!(
        "../fixtures/compatibility/byte_observations.json"
    ))
    .unwrap();
    for case in value["cases"].as_array().unwrap() {
        let expected = case["rust_expected_hex"].as_str().unwrap();
        let actual = compatibility_bytes(case["fixture"].as_str().unwrap());
        assert_eq!(
            hex(&actual),
            expected,
            "Rust bytes differ for {}",
            case["name"]
        );
        if let Some(node_expected) = case["node_expected_hex"].as_str() {
            if case["status"].as_str() == Some("divergent") {
                assert_ne!(
                    expected, node_expected,
                    "declared divergent case unexpectedly matches for {}",
                    case["name"]
                );
                assert!(case["divergence"].as_str().is_some());
            } else {
                assert_eq!(
                    expected, node_expected,
                    "Rust and Node bytes differ for {}",
                    case["name"]
                );
            }
        } else {
            assert!(case["uncapturable_reason"].as_str().is_some());
        }
        if let Some(observed) = node_bytes(case["name"].as_str().unwrap()) {
            assert_eq!(
                expected, observed,
                "dynamic Node bytes differ for {}",
                case["name"]
            );
        }
    }
}

#[test]
fn compatibility_token_matrix_matches_tokens_and_errors() {
    let value: Value = serde_json::from_str(include_str!(
        "../fixtures/compatibility/token_observations.json"
    ))
    .unwrap();
    for case in value["cases"].as_array().unwrap() {
        let bytes = decode_hex(case["hex"].as_str().unwrap());
        let observed = match case["name"].as_str().unwrap() {
            "done_success" => match tedious_rs::token::parser::parse_token(&bytes, 0, "7_4")
                .unwrap()
                .value
            {
                tedious_rs::token::parser::Token::Done(token) => vec![format!(
                    "token:DONE:token={:02x};more={};row_count={:?}",
                    token.token, token.more, token.row_count
                )],
                _ => panic!("DONE fixture parsed as another token"),
            },
            "return_status_zero" => match tedious_rs::token::parser::parse_token(&bytes, 0, "7_4")
                .unwrap()
                .value
            {
                tedious_rs::token::parser::Token::ReturnStatus(token) => {
                    vec![format!("token:RETURNSTATUS:value={}", token.value)]
                }
                _ => panic!("RETURNSTATUS fixture parsed as another token"),
            },
            "done_truncated" => match tedious_rs::token::parser::parse_token(&bytes, 0, "7_4") {
                Err(tedious_rs::token::parser::TokenParseError::NotEnoughData(_)) => {
                    vec!["error:token:not_enough_data".to_owned()]
                }
                _ => panic!("truncated token must be rejected"),
            },
            name => panic!("unknown compatibility token case: {name}"),
        };
        assert_eq!(
            observed,
            case["rust_expected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|item| item.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        );
        if let Some(node_expected) = case["node_expected"].as_array() {
            let expected = node_expected
                .iter()
                .map(|item| item.as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            assert_eq!(observed, expected);
            assert!(case["gap"].is_null());
        } else {
            assert!(case["node_expected"].is_null());
            assert!(case["gap"].as_str().is_some());
        }
    }
}

#[test]
fn type_fixture_is_byte_exact() {
    let value: Value = serde_json::from_str(include_str!("../fixtures/types/int.json")).unwrap();
    let encoded = int_parameter(value["value"].as_i64().unwrap());
    assert_eq!(
        hex(&encoded.type_info),
        value["type_info_hex"].as_str().unwrap()
    );
    assert_eq!(hex(&encoded.length), value["length_hex"].as_str().unwrap());
    assert_eq!(hex(&encoded.data), value["data_hex"].as_str().unwrap());
}

#[test]
fn success_callback_and_completion_order_matches_fixture() {
    let observations = Arc::new(Mutex::new(Vec::new()));
    let callback_observations = Arc::clone(&observations);
    let listener_observations = Arc::clone(&observations);
    let mut request = Request::new("SELECT 1", move |result| {
        callback_observations.lock().unwrap().push(
            if result.is_ok() {
                "callback:ok"
            } else {
                "callback:error"
            }
            .to_owned(),
        );
    });
    request.on(move |event| {
        if let Some(name) = event_name(event) {
            listener_observations.lock().unwrap().push(name);
        }
    });
    request.begin().expect("request should begin");
    request.finish();
    let observed = observations.lock().unwrap().clone();
    assert_expected("success_callback_before_completion", &observed);
    assert_dimension(
        "success_callback_before_completion",
        "final_state",
        &[state_name(request.state)],
    );
    assert_eq!(
        observed,
        node_request_observations("success").expect("Node observer must execute")
    );
}

#[test]
fn failure_error_callback_and_completion_order_matches_fixture() {
    let observations = Arc::new(Mutex::new(Vec::new()));
    let callback_observations = Arc::clone(&observations);
    let listener_observations = Arc::clone(&observations);
    let mut request = Request::new("SELECT 1", move |result| {
        let code = result
            .err()
            .and_then(|error| error.code)
            .unwrap_or_default();
        callback_observations
            .lock()
            .unwrap()
            .push(format!("callback:error:{code}"));
    });
    request.on(move |event| {
        if let Some(name) = event_name(event) {
            listener_observations.lock().unwrap().push(name);
        }
    });
    request.begin().expect("request should begin");
    request.fail(RequestError::new(
        "sanitized failure",
        Some("ERUNTIME".to_owned()),
    ));
    let observed = observations.lock().unwrap().clone();
    assert_expected("failure_event_callback_completion", &observed);
    assert_dimension(
        "failure_event_callback_completion",
        "final_state",
        &[state_name(request.state)],
    );
    assert_dimension(
        "failure_event_callback_completion",
        "error",
        &[format!(
            "code={}",
            request
                .error
                .as_ref()
                .and_then(|error| error.code.as_deref())
                .unwrap_or("absent")
        )],
    );
    assert_eq!(
        observed,
        node_request_observations("failure").expect("Node observer must execute")
    );
}

#[test]
fn invalid_second_begin_error_matches_fixture() {
    let mut request = Request::new("SELECT 1", |_| {});
    request.begin().expect("request should begin");
    let error = request.begin().expect_err("second begin must fail");
    let observed = vec![format!("error:{}", error.code.as_deref().unwrap_or(""))];
    assert_expected("invalid_second_begin", &observed);
    assert_dimension(
        "invalid_second_begin",
        "final_state",
        &[state_name(request.state)],
    );
    assert_dimension(
        "invalid_second_begin",
        "error",
        &[format!(
            "code={}",
            error.code.as_deref().unwrap_or("absent")
        )],
    );
}

#[test]
fn preparing_failure_gap_is_recorded_without_claiming_node_parity() {
    let observations = Arc::new(Mutex::new(Vec::new()));
    let listener_observations = Arc::clone(&observations);
    let mut request = Request::new("SELECT 1", |_| {});
    request.on(move |event| {
        if let Some(name) = event_name(event) {
            listener_observations.lock().unwrap().push(name);
        }
    });
    request.preparing = true;
    let error = RequestError::new("sanitized prepare failure", Some("EPREPARE".to_owned()));
    request.error = Some(error.clone());
    request.fail_preparing(error);
    let observed = observations.lock().unwrap().clone();
    assert_expected("preparing_failure_callback", &observed);
    assert_eq!(
        observed,
        node_request_observations("preparing_failure").expect("Node observer must execute")
    );
    assert_dimension(
        "preparing_failure_callback",
        "final_state",
        &[state_name(request.state)],
    );
    assert_dimension(
        "preparing_failure_callback",
        "error",
        &[format!(
            "code={}",
            request
                .error
                .as_ref()
                .and_then(|error| error.code.as_deref())
                .unwrap_or("absent")
        )],
    );
}
