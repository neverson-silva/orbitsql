use std::fs;

struct Mapping {
    node_path: &'static str,
    node_symbol: &'static str,
    rust_path: &'static str,
    rust_symbol: &'static str,
    test_path: &'static str,
    test_symbol: &'static str,
}

fn read_test_file(manifest: &str, test_path: &str) -> String {
    let direct = format!("{manifest}/{test_path}");
    if let Ok(content) = fs::read_to_string(&direct) {
        return content;
    }
    let file_name = test_path.rsplit('/').next().unwrap_or(test_path);
    ["unit", "integration", "compatibility"]
        .into_iter()
        .map(|directory| format!("{manifest}/tests/{directory}/{file_name}"))
        .find_map(|path| fs::read_to_string(path).ok())
        .unwrap_or_else(|| panic!("Rust test file must exist: {test_path}"))
}

const MAPPINGS: &[Mapping] = &[
    Mapping {
        node_path: "connection.ts",
        node_symbol: "class Connection",
        rust_path: "connection.rs",
        rust_symbol: "pub struct Connection",
        test_path: "tests/node_connection_facade.rs",
        test_symbol: "connect_returns_facade",
    },
    Mapping {
        node_path: "connection.ts",
        node_symbol: "connect(connectListener",
        rust_path: "connection.rs",
        rust_symbol: "pub async fn connect",
        test_path: "tests/tedious_connect_facade_parity.rs",
        test_symbol: "connect_facade_exposes",
    },
    Mapping {
        node_path: "connection.ts",
        node_symbol: "cancel()",
        rust_path: "connection.rs",
        rust_symbol: "pub fn cancel",
        test_path: "tests/cancellation_authenticated.rs",
        test_symbol: "authenticated_request_cancel",
    },
    Mapping {
        node_path: "connection.ts",
        node_symbol: "beginTransaction(",
        rust_path: "connection.rs",
        rust_symbol: "exec_transaction_payload",
        test_path: "tests/unit/unit_connection.rs",
        test_symbol: "transaction",
    },
    Mapping {
        node_path: "connection.ts",
        node_symbol: "prepare(",
        rust_path: "connection.rs",
        rust_symbol: "pub async fn prepare",
        test_path: "tests/unit/unit_connection.rs",
        test_symbol: "prepared_statement_operations",
    },
    Mapping {
        node_path: "connection.ts",
        node_symbol: "execute(",
        rust_path: "connection.rs",
        rust_symbol: "pub async fn execute",
        test_path: "tests/unit/unit_connection.rs",
        test_symbol: "prepared_statement_operations",
    },
    Mapping {
        node_path: "request.ts",
        node_symbol: "class Request",
        rust_path: "request.rs",
        rust_symbol: "pub struct Request",
        test_path: "tests/request_public_api.rs",
        test_symbol: "request",
    },
    Mapping {
        node_path: "request.ts",
        node_symbol: "addParameter(",
        rust_path: "request.rs",
        rust_symbol: "pub fn add_parameter",
        test_path: "tests/request_parameter_name_parity.rs",
        test_symbol: "parameter",
    },
    Mapping {
        node_path: "request.ts",
        node_symbol: "pause()",
        rust_path: "request.rs",
        rust_symbol: "pub fn pause",
        test_path: "tests/connection_request_pause_parity.rs",
        test_symbol: "pause",
    },
    Mapping {
        node_path: "request.ts",
        node_symbol: "resume()",
        rust_path: "request.rs",
        rust_symbol: "pub fn resume",
        test_path: "tests/connection_request_pause_parity.rs",
        test_symbol: "resume",
    },
    Mapping {
        node_path: "request.ts",
        node_symbol: "cancel()",
        rust_path: "request.rs",
        rust_symbol: "pub fn cancel",
        test_path: "tests/request_fail_parity.rs",
        test_symbol: "cancel",
    },
    Mapping {
        node_path: "token/handler.ts",
        node_symbol: "onInfoMessage(",
        rust_path: "token/handler.rs",
        rust_symbol: "on_info",
        test_path: "tests/unit/unit_token_handler.rs",
        test_symbol: "info_messages",
    },
    Mapping {
        node_path: "token/handler.ts",
        node_symbol: "onErrorMessage(",
        rust_path: "token/handler.rs",
        rust_symbol: "on_error",
        test_path: "tests/error_parity_audit.rs",
        test_symbol: "error",
    },
    Mapping {
        node_path: "token/handler.ts",
        node_symbol: "onRow(",
        rust_path: "token/handler.rs",
        rust_symbol: "on_row",
        test_path: "tests/unit/resultset_parity.rs",
        test_symbol: "rows_are_collected",
    },
    Mapping {
        node_path: "token/handler.ts",
        node_symbol: "onDone(",
        rust_path: "token/handler.rs",
        rust_symbol: "on_done",
        test_path: "tests/unit/unit_token_handler.rs",
        test_symbol: "done",
    },
    Mapping {
        node_path: "token/env-change-token-parser.ts",
        node_symbol: "function envChangeParser",
        rust_path: "token/parser.rs",
        rust_symbol: "fn env_change",
        test_path: "tests/unit/unit_token_parser.rs",
        test_symbol: "envchange",
    },
    Mapping {
        node_path: "token/stream-parser.ts",
        node_symbol: "readToken(",
        rust_path: "token/parser.rs",
        rust_symbol: "pub fn parse_token",
        test_path: "tests/unit/token_spec07.rs",
        test_symbol: "stream",
    },
    Mapping {
        node_path: "token/done-token-parser.ts",
        node_symbol: "function doneParser",
        rust_path: "token/done_token_parser.rs",
        rust_symbol: "pub fn parse",
        test_path: "tests/unit/unit_token_parser.rs",
        test_symbol: "done_variants",
    },
    Mapping {
        node_path: "token/row-token-parser.ts",
        node_symbol: "function rowParser",
        rust_path: "token/row_token_parser.rs",
        rust_symbol: "pub fn parse",
        test_path: "tests/unit/unit_rows.rs",
        test_symbol: "row",
    },
    Mapping {
        node_path: "value-parser.ts",
        node_symbol: "function readValue",
        rust_path: "value_parser.rs",
        rust_symbol: "pub fn read_value",
        test_path: "tests/unit/unit_value_parser_parity.rs",
        test_symbol: "value",
    },
    Mapping {
        node_path: "metadata-parser.ts",
        node_symbol: "function readMetadata",
        rust_path: "metadata_parser.rs",
        rust_symbol: "pub fn read_metadata",
        test_path: "tests/unit/unit_metadata_parser.rs",
        test_symbol: "metadata",
    },
    Mapping {
        node_path: "collation.ts",
        node_symbol: "class Collation",
        rust_path: "collation.rs",
        rust_symbol: "pub struct Collation",
        test_path: "tests/unit/unit_collation.rs",
        test_symbol: "collation",
    },
    Mapping {
        node_path: "data-type.ts",
        node_symbol: "const TYPES",
        rust_path: "data_type.rs",
        rust_symbol: "pub const TYPES",
        test_path: "tests/unit/unit_data_types.rs",
        test_symbol: "registry_contains_every_source_data_type",
    },
    Mapping {
        node_path: "data-types/tvp.ts",
        node_symbol: "const TVP",
        rust_path: "payloads.rs",
        rust_symbol: "pub struct TvpPayload",
        test_path: "tests/tvp_public_api_parity.rs",
        test_symbol: "tvp",
    },
    Mapping {
        node_path: "bulk-load.ts",
        node_symbol: "addColumn(",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn add_column",
        test_path: "tests/unit/unit_bulk_load_api.rs",
        test_symbol: "add_column",
    },
    Mapping {
        node_path: "bulk-load.ts",
        node_symbol: "cancel()",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn cancel",
        test_path: "tests/bulk_load_request_lifecycle.rs",
        test_symbol: "canceled",
    },
    Mapping {
        node_path: "transaction.ts",
        node_symbol: "beginPayload(",
        rust_path: "transaction.rs",
        rust_symbol: "pub fn try_begin_payload",
        test_path: "tests/unit/unit_connection.rs",
        test_symbol: "transaction",
    },
    Mapping {
        node_path: "incoming-message-stream.ts",
        node_symbol: "processBufferedData(",
        rust_path: "incoming_message_stream.rs",
        rust_symbol: "pub fn process_buffered_data",
        test_path: "tests/unit/unit_message_streams.rs",
        test_symbol: "processes_buffered_data",
    },
    Mapping {
        node_path: "outgoing-message-stream.ts",
        node_symbol: "_write(",
        rust_path: "outgoing_message_stream.rs",
        rust_symbol: "pub fn write_message",
        test_path: "tests/stream_backpressure_drain_parity.rs",
        test_symbol: "outgoing_drain",
    },
    Mapping {
        node_path: "message-io.ts",
        node_symbol: "readMessage(",
        rust_path: "transport.rs",
        rust_symbol: "pub async fn read_message",
        test_path: "tests/transport_read_message_parity.rs",
        test_symbol: "incremental_read_exposes",
    },
    Mapping {
        node_path: "message-io.ts",
        node_symbol: "sendMessage(",
        rust_path: "transport.rs",
        rust_symbol: "pub async fn send_message",
        test_path: "tests/transport_write_cancel_parity.rs",
        test_symbol: "cancellation_does_not_interrupt",
    },
    Mapping {
        node_path: "connector.ts",
        node_symbol: "connectInParallel",
        rust_path: "connector.rs",
        rust_symbol: "pub async fn connect_in_parallel",
        test_path: "tests/unit/unit_connector.rs",
        test_symbol: "parallel",
    },
    Mapping {
        node_path: "instance-lookup.ts",
        node_symbol: "parseBrowserResponse",
        rust_path: "instance_lookup.rs",
        rust_symbol: "pub fn parse_browser_response",
        test_path: "tests/unit/unit_instance_lookup.rs",
        test_symbol: "browser_response",
    },
    Mapping {
        node_path: "sqlbatch-payload.ts",
        node_symbol: "class SqlBatchPayload",
        rust_path: "payloads.rs",
        rust_symbol: "pub struct SqlBatchPayload",
        test_path: "tests/unit/unit_payloads.rs",
        test_symbol: "serializes_sql_batch",
    },
    Mapping {
        node_path: "sqlbatch-payload.ts",
        node_symbol: "toString(indent = '')",
        rust_path: "payloads.rs",
        rust_symbol: "pub fn to_string",
        test_path: "tests/unit/unit_payloads.rs",
        test_symbol: "formats_sql_batch_diagnostic",
    },
    Mapping {
        node_path: "rpcrequest-payload.ts",
        node_symbol: "class RpcRequestPayload",
        rust_path: "payloads.rs",
        rust_symbol: "pub struct RpcRequestPayload",
        test_path: "tests/unit/unit_payloads.rs",
        test_symbol: "serializes_rpc_procedure_parameters",
    },
    Mapping {
        node_path: "rpcrequest-payload.ts",
        node_symbol: "toString(indent = '')",
        rust_path: "payloads.rs",
        rust_symbol: "pub fn to_string",
        test_path: "tests/unit/unit_payloads.rs",
        test_symbol: "serializes_rpc_empty",
    },
    Mapping {
        node_path: "bulk-load-payload.ts",
        node_symbol: "export class BulkLoadPayload",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn payload",
        test_path: "tests/unit/unit_bulk_load.rs",
        test_symbol: "serializes_bulk_metadata",
    },
    Mapping {
        node_path: "bulk-load-payload.ts",
        node_symbol: "toString(indent = '')",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn to_string",
        test_path: "tests/unit/unit_bulk_load_api.rs",
        test_symbol: "exposes_node_payload_helpers",
    },
    Mapping {
        node_path: "bulk-load.ts",
        node_symbol: "getOptionsSql()",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn get_options_sql",
        test_path: "tests/unit/unit_bulk_load_api.rs",
        test_symbol: "builds_bulk_insert_and_table_sql",
    },
    Mapping {
        node_path: "bulk-load.ts",
        node_symbol: "getBulkInsertSql()",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn get_bulk_insert_sql",
        test_path: "tests/unit/unit_bulk_load_api.rs",
        test_symbol: "builds_bulk_insert_and_table_sql",
    },
    Mapping {
        node_path: "bulk-load.ts",
        node_symbol: "getTableCreationSql()",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn get_table_creation_sql",
        test_path: "tests/unit/unit_bulk_load_api.rs",
        test_symbol: "builds_bulk_insert_and_table_sql",
    },
    Mapping {
        node_path: "bulk-load.ts",
        node_symbol: "getColMetaData()",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn get_col_metadata",
        test_path: "tests/unit/unit_bulk_load_api.rs",
        test_symbol: "exposes_node_payload_helpers",
    },
    Mapping {
        node_path: "bulk-load.ts",
        node_symbol: "createDoneToken()",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn create_done_token",
        test_path: "tests/unit/unit_bulk_load_api.rs",
        test_symbol: "creates_done_token",
    },
    Mapping {
        node_path: "transaction.ts",
        node_symbol: "commitPayload(",
        rust_path: "transaction.rs",
        rust_symbol: "pub fn try_commit_payload",
        test_path: "tests/unit/unit_connection.rs",
        test_symbol: "transaction",
    },
    Mapping {
        node_path: "transaction.ts",
        node_symbol: "rollbackPayload(",
        rust_path: "transaction.rs",
        rust_symbol: "pub fn try_rollback_payload",
        test_path: "tests/unit/unit_connection.rs",
        test_symbol: "transaction",
    },
    Mapping {
        node_path: "transaction.ts",
        node_symbol: "savePayload(",
        rust_path: "transaction.rs",
        rust_symbol: "pub fn try_save_payload",
        test_path: "tests/unit/unit_connection.rs",
        test_symbol: "transaction",
    },
    Mapping {
        node_path: "all-headers.ts",
        node_symbol: "writeToTrackingBuffer(",
        rust_path: "all_headers.rs",
        rust_symbol: "pub fn write_to_tracking_buffer",
        test_path: "tests/unit/unit_connection.rs",
        test_symbol: "transaction",
    },
    Mapping {
        node_path: "sender.ts",
        node_symbol: "sendInParallel(",
        rust_path: "sender.rs",
        rust_symbol: "pub async fn send_in_parallel",
        test_path: "tests/unit/unit_sender.rs",
        test_symbol: "sends_udp_request",
    },
    Mapping {
        node_path: "sender.ts",
        node_symbol: "sendMessage(",
        rust_path: "sender.rs",
        rust_symbol: "pub async fn send_message",
        test_path: "tests/unit/transport_lookup_cancellation.rs",
        test_symbol: "instance_lookup_uses_injected_lookup",
    },
    Mapping {
        node_path: "connector.ts",
        node_symbol: "connectInSequence",
        rust_path: "connector.rs",
        rust_symbol: "pub async fn connect_in_sequence",
        test_path: "tests/unit/unit_connector.rs",
        test_symbol: "sequence_connect",
    },
    Mapping {
        node_path: "connector.ts",
        node_symbol: "lookupAllAddresses",
        rust_path: "connector.rs",
        rust_symbol: "pub async fn lookup_all_addresses",
        test_path: "tests/unit/unit_connector.rs",
        test_symbol: "resolves_numeric",
    },
    Mapping {
        node_path: "instance-lookup.ts",
        node_symbol: "instanceLookup(",
        rust_path: "instance_lookup.rs",
        rust_symbol: "pub async fn lookup_instance",
        test_path: "tests/unit/unit_instance_lookup.rs",
        test_symbol: "empty_server_name",
    },
    Mapping {
        node_path: "message.ts",
        node_symbol: "class Message",
        rust_path: "message.rs",
        rust_symbol: "pub struct Message",
        test_path: "tests/unit/unit_message_streams.rs",
        test_symbol: "Message::new",
    },
];

#[test]
fn every_scoped_symbol_has_source_destination_and_test_evidence() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    assert!(MAPPINGS.len() >= 30);
    for mapping in MAPPINGS {
        let node = fs::read_to_string(format!(
            "{manifest}/../tedious-node/src/{}",
            mapping.node_path
        ))
        .expect("Node source file must exist");
        assert!(
            node.contains(mapping.node_symbol),
            "Node symbol is absent: {}",
            mapping.node_symbol
        );
        let rust = fs::read_to_string(format!("{manifest}/src/{}", mapping.rust_path))
            .expect("Rust source file must exist");
        assert!(
            rust.contains(mapping.rust_symbol),
            "Rust symbol is absent: {}",
            mapping.rust_symbol
        );
        let test = read_test_file(manifest, mapping.test_path);
        assert!(
            test.contains(mapping.test_symbol),
            "Test evidence is absent: {}",
            mapping.test_symbol
        );
    }
}
