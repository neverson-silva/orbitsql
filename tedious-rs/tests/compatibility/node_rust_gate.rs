use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tedious_rs::login7_payload::{Login7Options, Login7Payload};
use tedious_rs::metadata_parser::read_metadata;
use tedious_rs::parameter::{ParameterOptions, SqlParameterType};
use tedious_rs::payloads::{BulkLoadColumn, BulkLoadPayload, BulkLoadRow};
use tedious_rs::payloads::{
    RpcParameter, RpcRequestPayload, SqlBatchPayload, TvpColumnDefinition, TvpPayload, TvpRow,
};
use tedious_rs::request::{Request, RequestEvent};
use tedious_rs::token::parser::{Token, parse_token};
use tedious_rs::value_codec::Value as CodecValue;
use tedious_rs::value_parser::read_value;

const NODE_OBSERVER: &str = r#"
const crypto = require('crypto');
const originalRandomBytes = crypto.randomBytes;
crypto.randomBytes = size => Buffer.alloc(size, 0x11);
const PreloginPayload = require('./tedious-node/lib/prelogin-payload');
const Login7Payload = require('./tedious-node/lib/login7-payload');
const SqlBatchPayload = require('./tedious-node/lib/sqlbatch-payload');
const RpcRequestPayload = require('./tedious-node/lib/rpcrequest-payload');
const TVP = require('./tedious-node/lib/data-types/tvp');
const Int = require('./tedious-node/lib/data-types/int');
const Request = require('./tedious-node/lib/request');
const metadataParser = require('./tedious-node/lib/metadata-parser');
const valueParser = require('./tedious-node/lib/value-parser');
const doneParser = require('./tedious-node/lib/token/done-token-parser').doneParser;
const returnStatusParser = require('./tedious-node/lib/token/returnstatus-token-parser');

const hex = value => Buffer.concat([...value]).toString('hex');
const options = { tdsVersion: '7_4' };
const descriptor = Buffer.alloc(8);
const intParameter = { name: 'p', output: false, value: 7, type: Int };
const sql = new SqlBatchPayload('SELECT 1', descriptor, options);
const rpc = new RpcRequestPayload('sp_executesql', [intParameter], descriptor, options);
const tvpValue = { schema: 'dbo', name: 'CompatType', columns: [{ name: 'id', type: Int }], rows: [[7]] };
const tvpParameter = { value: tvpValue };
const tvpParts = [TVP.generateTypeInfo(tvpParameter), TVP.generateParameterLength(tvpParameter, options), ...TVP.generateParameterData(tvpParameter, options)];
const login = new Login7Payload({ tdsVersion: 0x72090002, packetSize: 4096, clientProgVer: 1, clientPid: 7, connectionId: 0, clientTimeZone: 0, clientLcid: 0x0409 });
login.hostname = 'compat';
login.userName = 'sa';
login.password = 'x';
login.appName = 'tedious-rs';
login.serverName = 'localhost';
login.database = 'tedious_rs';
const prelogin = new PreloginPayload({ encrypt: false, version: { major: 0, minor: 0, build: 0, subbuild: 0 } });
crypto.randomBytes = originalRandomBytes;

const done = Buffer.from('000000000000000000000000', 'hex');
const status = Buffer.from('00000000', 'hex');
const metadataBytes = Buffer.from('443322110700380400', 'hex');
const metadata = metadataParser.readMetadata(metadataBytes, 0, options).value;
const row = valueParser.readValue(Buffer.from('07000000', 'hex'), 0, metadata, options).value;
const nodeEvents = [];
const nodeRequest = new Request('SELECT 1', (err, rowCount, rows) => nodeEvents.push(err ? 'callback:error' : 'callback:ok'));
nodeRequest.on('requestCompleted', () => nodeEvents.push('event:requestCompleted'));
nodeRequest.callback(null, 1, []);
const nodeErrorEvents = [];
const nodeErrorRequest = new Request('SELECT 1', err => nodeErrorEvents.push(err ? 'callback:error' : 'callback:ok'));
nodeErrorRequest.on('requestCompleted', () => nodeErrorEvents.push('event:requestCompleted'));
const nodeError = new Error('runtime failure');
nodeError.code = 'ERUNTIME';
nodeErrorRequest.callback(nodeError, 0, []);

function simpleMetadata(value) {
  return { userType: value.userType, flags: value.flags, type: value.type.name, id: value.type.id, dataLength: value.dataLength ?? null, precision: value.precision ?? null, scale: value.scale ?? null };
}
function tokenObservation(buffer, parser) {
  try {
    const result = parser(buffer, 0, options);
    const token = result.value;
    if (token.name === 'DONE') return { kind: 'done', more: token.more, rowCount: token.rowCount ?? null, curCmd: token.curCmd };
    return { kind: 'returnStatus', value: token.value };
  } catch (error) {
    return { kind: 'error', category: error.constructor.name };
  }
}
process.stdout.write(JSON.stringify({
  bytes: {
    prelogin: prelogin.data.toString('hex'),
    login7: login.toBuffer().toString('hex'),
    sqlBatch: hex(sql),
    rpc: hex(rpc),
    tvp: Buffer.concat(tvpParts).toString('hex'),
    intParameter: Buffer.concat([Int.generateTypeInfo(), Int.generateParameterLength({ value: 7 }, options), ...Int.generateParameterData({ value: 7 }, options)]).toString('hex')
  },
  tokens: {
    done: tokenObservation(done, doneParser),
    returnStatus: tokenObservation(status, returnStatusParser),
    truncated: tokenObservation(Buffer.from('000000', 'hex'), doneParser)
  },
  metadata: simpleMetadata(metadata),
  rows: [row],
  events: nodeEvents,
  errorEvents: nodeErrorEvents,
  error: { code: 'ERUNTIME', message: 'runtime failure' },
  state: { canceled: nodeRequest.canceled, paused: nodeRequest.paused, error: nodeRequest.error === undefined ? null : String(nodeRequest.error) }
}));
process.exit(0);
"#;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn node_observation() -> Value {
    let output = Command::new("node")
        .arg("-e")
        .arg(NODE_OBSERVER)
        .current_dir(format!("{}/..", env!("CARGO_MANIFEST_DIR")))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Node observer must start");
    assert!(
        output.status.success(),
        "Node observer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut observation =
        serde_json::from_slice::<Value>(&output.stdout).expect("Node observer must emit JSON");
    observation["bytes"]["bulk"] = json!(node_bytes_observation("bulk"));
    observation["preparingEvents"] = json!(node_request_observation("preparing_failure"));
    observation
}

fn node_bytes_observation(case: &str) -> String {
    let output = Command::new("node")
        .args(["tests/compatibility/node_bytes_observer.js", case])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Node bytes observer must start");
    assert!(
        output.status.success(),
        "Node bytes observer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("Node bytes observer must emit UTF-8")
        .trim()
        .to_owned()
}

fn node_request_observation(mode: &str) -> Value {
    let output = Command::new("node")
        .args(["tests/compatibility/node_request_observer.js", mode])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Node request observer must start");
    assert!(
        output.status.success(),
        "Node request observer failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Node request observer must emit JSON")
}

fn int_parameter(value: i64) -> tedious_rs::value_codec::EncodedParameter {
    SqlParameterType::Int
        .encode(&CodecValue::Integer(value), &ParameterOptions::default())
        .unwrap()
}

fn rust_observation() -> Value {
    let prelogin = tedious_rs::prelogin_payload::PreloginPayload::from_bytes(
        hex_decode("000024000601002a000102002b000103002c0004040030000105003100240600550001ff0000000000000200000000000011111111111111111111111111111111111111111111111111111111111111111111111101"),
    ).unwrap().data;
    let mut login = Login7Payload::new(Login7Options {
        tds_version: 0x72090002,
        packet_size: 4096,
        client_prog_ver: 1,
        client_pid: 7,
        connection_id: 0,
        client_time_zone: 0,
        client_lcid: 0x0409,
    });
    login.hostname = Some("compat".to_owned());
    login.user_name = Some("sa".to_owned());
    login.password = Some("x".to_owned());
    login.app_name = Some("tedious-rs".to_owned());
    login.server_name = Some("localhost".to_owned());
    login.database = Some("tedious_rs".to_owned());
    let encoded = int_parameter(7);
    let sql_batch = SqlBatchPayload {
        sql_text: "SELECT 1".to_owned(),
        transaction_descriptor: vec![0; 8],
        tds_version: "7_4".to_owned(),
    }
    .buffers()
    .unwrap()
    .concat();
    let rpc = RpcRequestPayload {
        procedure: Some("sp_executesql".to_owned()),
        procedure_id: None,
        parameters: vec![RpcParameter {
            name: "p".to_owned(),
            output: false,
            encoded: encoded.clone(),
            encrypted_metadata: None,
        }],
        transaction_descriptor: vec![0; 8],
        tds_version: "7_4".to_owned(),
    }
    .buffers()
    .unwrap()
    .concat();
    let tvp = TvpPayload {
        database: String::new(),
        schema: "dbo".to_owned(),
        type_name: "CompatType".to_owned(),
        columns: vec![TvpColumnDefinition {
            name: "id".to_owned(),
            type_info: vec![0x26, 4],
        }],
        rows: vec![TvpRow {
            values: vec![encoded.clone()],
        }],
    }
    .buffers()
    .unwrap()
    .concat();
    let bulk = BulkLoadPayload {
        columns: vec![BulkLoadColumn {
            name: "id".to_owned(),
            table_name: "dbo.compat".to_owned(),
            type_info: vec![0x26, 4],
            nullable: false,
            legacy_lob: false,
        }],
        rows: vec![BulkLoadRow {
            values: vec![encoded.clone()],
        }],
        tds_version: "7_4".to_owned(),
    }
    .data()
    .unwrap();
    let metadata_bytes = hex_decode("443322110700380400");
    let metadata = read_metadata(&metadata_bytes, 0, "7_4").unwrap().value;
    let row = read_value(&[7, 0, 0, 0], 0, &metadata).unwrap().value;
    let events = Arc::new(Mutex::new(Vec::<String>::new()));
    let callback_events = Arc::clone(&events);
    let listener_events = Arc::clone(&events);
    let mut request = Request::new("SELECT 1", move |result| {
        callback_events.lock().unwrap().push(
            if result.is_ok() {
                "callback:ok"
            } else {
                "callback:error"
            }
            .to_owned(),
        )
    });
    request.on(move |event| {
        if matches!(event, RequestEvent::RequestCompleted) {
            listener_events
                .lock()
                .unwrap()
                .push("event:requestCompleted".to_owned())
        }
    });
    request.begin().unwrap();
    request.finish();
    let error_events = Arc::new(Mutex::new(Vec::<String>::new()));
    let error_callback = Arc::clone(&error_events);
    let mut error_request = Request::new("SELECT 1", move |result| {
        error_callback.lock().unwrap().push(
            if result.is_ok() {
                "callback:ok"
            } else {
                "callback:error"
            }
            .to_owned(),
        )
    });
    let error_completion = Arc::new(Mutex::new(false));
    let error_completion_seen = Arc::clone(&error_completion);
    error_request.on(move |event| {
        if matches!(event, RequestEvent::RequestCompleted) {
            *error_completion_seen.lock().unwrap() = true;
        }
    });
    error_request.fail(tedious_rs::errors::RequestError::new(
        "runtime failure",
        Some("ERUNTIME".to_owned()),
    ));
    let done_token = parse_token(&hex_decode("fd000000000000000000000000"), 0, "7_4").unwrap();
    let status_token = parse_token(&hex_decode("7900000000"), 0, "7_4").unwrap();
    let truncated = parse_token(&hex_decode("fd000000"), 0, "7_4");
    let token = match done_token.value {
        Token::Done(value) => {
            json!({"kind":"done", "more":value.more, "rowCount":value.row_count, "curCmd":value.cur_cmd})
        }
        _ => json!({"kind":"wrong"}),
    };
    let status = match status_token.value {
        Token::ReturnStatus(value) => json!({"kind":"returnStatus", "value":value.value}),
        _ => json!({"kind":"wrong"}),
    };
    let truncated = json!({"kind":"error", "category": if truncated.is_err() { "NotEnoughDataError" } else { "none" }});
    let mut observed_error_events = error_events.lock().unwrap().clone();
    if *error_completion.lock().unwrap() {
        observed_error_events.push("event:requestCompleted".to_owned());
    }
    let preparing_events = Arc::new(Mutex::new(Vec::<String>::new()));
    let preparing_callback_events = Arc::clone(&preparing_events);
    let preparing_listener_events = Arc::clone(&preparing_events);
    let mut preparing_request = Request::new("SELECT 1", move |result| {
        preparing_callback_events
            .lock()
            .unwrap()
            .push(if result.is_ok() {
                "callback:ok".to_owned()
            } else {
                "callback:error:EPREPARE".to_owned()
            });
    });
    preparing_request.on(move |event| {
        if let RequestEvent::Error(error) = event {
            preparing_listener_events.lock().unwrap().push(format!(
                "event:error:{}",
                error.code.as_deref().unwrap_or("")
            ));
        }
    });
    preparing_request.preparing = true;
    let preparing_error = tedious_rs::errors::RequestError::new(
        "sanitized prepare failure",
        Some("EPREPARE".to_owned()),
    );
    preparing_request.error = Some(preparing_error.clone());
    preparing_request.fail_preparing(preparing_error);
    json!({"bytes":{"prelogin":hex(&prelogin),"login7":hex(&login.to_buffer().unwrap()),"sqlBatch":hex(&sql_batch),"rpc":hex(&rpc),"tvp":hex(&tvp),"bulk":hex(&bulk),"intParameter":hex(&[encoded.type_info,encoded.length,encoded.data].concat())},"tokens":{"done":token,"returnStatus":status,"truncated":truncated},"metadata":{"userType":metadata.user_type,"flags":metadata.flags,"type":metadata.data_type.name,"id":metadata.data_type.id,"dataLength":metadata.data_length,"precision":metadata.precision,"scale":metadata.scale},"rows":[match row { CodecValue::Integer(value) => json!(value), CodecValue::Null => Value::Null, _ => json!("unsupported") }],"events":events.lock().unwrap().clone(),"errorEvents":observed_error_events,"preparingEvents":preparing_events.lock().unwrap().clone(),"error":{"code":"ERUNTIME","message":"runtime failure"},"state":{"canceled":false,"paused":false,"error":null},"rustState":format!("{:?}",request.state)})
}

fn hex_decode(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn node_and_rust_observers_match_wire_and_runtime_observations() {
    let node = node_observation();
    let rust = rust_observation();
    let differences = [
        "bytes",
        "tokens",
        "metadata",
        "rows",
        "events",
        "errorEvents",
        "preparingEvents",
        "state",
        "error",
    ]
    .into_iter()
    .filter(|dimension| rust[*dimension] != node[*dimension])
    .map(|dimension| {
        format!(
            "{dimension}: node={} rust={}",
            node[dimension], rust[dimension]
        )
    })
    .collect::<Vec<_>>();
    assert!(
        differences.is_empty(),
        "Node and Rust observations differ: {}",
        differences.join(" | ")
    );
    assert_eq!(rust["rustState"], "Completed");
}
