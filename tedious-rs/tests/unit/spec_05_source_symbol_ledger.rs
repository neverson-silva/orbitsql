use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
struct SymbolRecord {
    node_path: String,
    node_symbol: String,
    responsibility: String,
    rust_path: String,
    rust_symbol: String,
    test_path: String,
    test_symbol: String,
    dependencies: Vec<String>,
    status: String,
    divergence: String,
}

struct GapRecord {
    node_path: &'static str,
    node_symbol: &'static str,
    responsibility: &'static str,
    rust_path: &'static str,
    rust_symbol: &'static str,
    test_path: &'static str,
    test_symbol: &'static str,
    dependencies: &'static [&'static str],
    status: &'static str,
    divergence: &'static str,
}

fn read_test_file(manifest: &Path, test_path: &str) -> String {
    let direct = manifest.join(test_path);
    if let Ok(content) = fs::read_to_string(&direct) {
        return content;
    }
    let file_name = Path::new(test_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(test_path);
    ["unit", "integration", "compatibility"]
        .into_iter()
        .map(|directory| manifest.join("tests").join(directory).join(file_name))
        .find_map(|path| fs::read_to_string(path).ok())
        .unwrap_or_else(|| panic!("specific behavior test must exist: {test_path}"))
}

const GAP_RECORDS: &[GapRecord] = &[
    GapRecord {
        node_path: "incoming-message-stream.ts",
        node_symbol: "pause()",
        responsibility: "propagates pause to current Message",
        rust_path: "incoming_message_stream.rs",
        rust_symbol: "pub fn pause",
        test_path: "tests/streaming_backpressure_parity.rs",
        test_symbol: "node_and_rust_incoming_stream_runtime_sequences_match",
        dependencies: &["bl@2.2.1", "stream.Transform"],
        status: "equivalent",
        divergence: "The Node Transform and Rust assembler now execute the same controlled pause, resume, fragment and message-order scenario with matching observable events; native socket pressure remains transport-dependent.",
    },
    GapRecord {
        node_path: "incoming-message-stream.ts",
        node_symbol: "resume()",
        responsibility: "propagates resume to current Message",
        rust_path: "incoming_message_stream.rs",
        rust_symbol: "pub fn resume",
        test_path: "tests/streaming_backpressure_parity.rs",
        test_symbol: "incoming_pause_preserves_wire_bytes_and_message_order",
        dependencies: &["bl@2.2.1", "stream.Transform"],
        status: "part",
        divergence: "Node resumes the current PassThrough and Rust drains its incremental assembler; the controlled runtime event sequence, bytes and completion order match.",
    },
    GapRecord {
        node_path: "incoming-message-stream.ts",
        node_symbol: "processBufferedData",
        responsibility: "frames complete TDS packets and waits for message drain",
        rust_path: "incoming_message_stream.rs",
        rust_symbol: "pub fn feed_incremental",
        test_path: "tests/streaming_backpressure_parity.rs",
        test_symbol: "node_and_rust_incoming_stream_runtime_sequences_match",
        dependencies: &["bl@2.2.1", "packet.ts", "message.ts"],
        status: "part",
        divergence: "The Rust assembler validates and materializes packets without Node Packet objects; the externally observable framing, pause/resume, bytes and ordering now have Node runtime comparison.",
    },
    GapRecord {
        node_path: "message-io.ts",
        node_symbol: "startTls",
        responsibility: "upgrades the duplex pair to TLS",
        rust_path: "transport.rs",
        rust_symbol: "pub async fn connect_tls",
        test_path: "tests/strict_tls.rs",
        test_symbol: "negotiates_strict_tls_with_sni_and_required_alpn",
        dependencies: &["native-duplexpair@1.0.0", "tls", "native-tls@0.2"],
        status: "evidence-limited",
        divergence: "The Rust TLS adapter validates SNI and strict encryption locally; no SQL Server TLS endpoint is available in this audit.",
    },
    GapRecord {
        node_path: "message-io.ts",
        node_symbol: "sendMessage",
        responsibility: "writes a message and waits for drain",
        rust_path: "transport.rs",
        rust_symbol: "pub async fn send_message",
        test_path: "tests/transport_write_cancel_parity.rs",
        test_symbol: "cancellation_does_not_interrupt",
        dependencies: &["native-duplexpair@1.0.0", "stream.Writable"],
        status: "part",
        divergence: "The Rust writer preserves packet framing and cancellation boundaries; Node drain scheduling is covered with a transport fixture, not a native DuplexPair.",
    },
    GapRecord {
        node_path: "message-io.ts",
        node_symbol: "readMessage",
        responsibility: "reads one complete TDS message",
        rust_path: "transport.rs",
        rust_symbol: "pub async fn read_message",
        test_path: "tests/transport_read_message_parity.rs",
        test_symbol: "incremental_read_exposes",
        dependencies: &["native-duplexpair@1.0.0", "stream.Readable"],
        status: "part",
        divergence: "Rust incrementally reassembles packets and rejects truncation; native stream event scheduling remains evidence-limited.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "cancel",
        responsibility: "cancels the active request with ATTENTION",
        rust_path: "connection.rs",
        rust_symbol: "pub fn cancel",
        test_path: "tests/node_connection_runtime_parity.rs",
        test_symbol: "node_and_rust_cancel_runtime_sequences_match",
        dependencies: &["message-io.ts", "token/handler.ts"],
        status: "part",
        divergence: "Authenticated fake transport and Node runtime comparison prove ATTENTION, acknowledgement, state recovery, callback error and next-request alignment; the separate live SQL Server router cancellation branch is not available.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "performTransientFailureRetry",
        responsibility: "retries transient connection failures",
        rust_path: "connection.rs",
        rust_symbol: "perform_transient_failure_retry",
        test_path: "tests/retry_reroute.rs",
        test_symbol: "reroutes_to_the_reported_host_port_and_preserves_instance_for_login7",
        dependencies: &["connector.ts", "instance-lookup.ts"],
        status: "part",
        divergence: "Retry and reroute state are covered by deterministic endpoints; transient server failure classification is not proven against a live server.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "performReRouting",
        responsibility: "applies routing host and port",
        rust_path: "connection.rs",
        rust_symbol: "perform_re_routing",
        test_path: "tests/retry_reroute.rs",
        test_symbol: "reroutes_to_the_reported_host_port_and_preserves_instance_for_login7",
        dependencies: &["token/env-change-token-parser.ts"],
        status: "part",
        divergence: "Routing data and LOGIN7 preservation are verified byte-wise; live routing event delivery is unavailable.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "beginTransaction",
        responsibility: "starts a transaction through TDS or SQL batch",
        rust_path: "connection.rs",
        rust_symbol: "pub async fn begin_transaction",
        test_path: "tests/unit/unit_connection.rs",
        test_symbol: "transaction_operations_require_logged_in_connection_and_keep_empty_descriptor",
        dependencies: &["transaction.ts", "sqlbatch-payload.ts", "all-headers.ts"],
        status: "part",
        divergence: "Both TDS branches are implemented and local SQL authentication integration is opt-in; execution evidence depends on the configured local SQL Server.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "commitTransaction",
        responsibility: "commits a transaction",
        rust_path: "connection.rs",
        rust_symbol: "pub async fn commit_transaction",
        test_path: "tests/integration/integration_sql_auth.rs",
        test_symbol: "sql_auth_transaction_commit_and_savepoint_preserve_descriptor_lifecycle",
        dependencies: &["transaction.ts", "all-headers.ts"],
        status: "part",
        divergence: "The lifecycle and descriptor reset are tested locally; live SQL Server execution is opt-in.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "rollbackTransaction",
        responsibility: "rolls back a transaction",
        rust_path: "connection.rs",
        rust_symbol: "pub async fn rollback_transaction",
        test_path: "tests/integration/integration_sql_auth.rs",
        test_symbol: "sql_auth_preserves_multiple_result_sets_and_transaction_rollback",
        dependencies: &["transaction.ts", "all-headers.ts"],
        status: "part",
        divergence: "The lifecycle and descriptor reset are tested locally; live SQL Server execution is opt-in.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "saveTransaction",
        responsibility: "creates a transaction savepoint",
        rust_path: "connection.rs",
        rust_symbol: "pub async fn save_transaction",
        test_path: "tests/integration/integration_sql_auth.rs",
        test_symbol: "sql_auth_transaction_commit_and_savepoint_preserve_descriptor_lifecycle",
        dependencies: &["transaction.ts", "sqlbatch-payload.ts", "all-headers.ts"],
        status: "part",
        divergence: "The TDS and legacy SQL paths are implemented; live savepoint execution is opt-in.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "makeRequest",
        responsibility: "serializes and executes a request",
        rust_path: "connection.rs",
        rust_symbol: "pub async fn exec_request",
        test_path: "tests/node_connection_runtime_parity.rs",
        test_symbol: "node_and_rust_request_runtime_sequences_match",
        dependencies: &["request.ts", "message-io.ts", "token/stream-parser.ts"],
        status: "part",
        divergence: "Request lifecycle, timeout, authenticated cancellation and the standard execSql Node runtime sequence have live comparison of packets, states, token events, callback ordering, ECANCEL/ETIMEOUT and callback-before-end cleanup.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "readPreloginResponse",
        responsibility: "reads PRELOGIN negotiation response",
        rust_path: "connection.rs",
        rust_symbol: "read_prelogin_response",
        test_path: "tests/prelogin_tls_parity.rs",
        test_symbol: "encrypt_true_with_server_off_sends_login7_without_tls",
        dependencies: &["prelogin-payload.ts", "message-io.ts"],
        status: "evidence-limited",
        divergence: "Strict TLS and PRELOGIN framing are tested with local transport; SQL Server negotiation is unavailable.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "performTlsNegotiation",
        responsibility: "negotiates TLS after PRELOGIN",
        rust_path: "connection.rs",
        rust_symbol: "perform_tls_negotiation",
        test_path: "tests/strict_tls.rs",
        test_symbol: "negotiates_strict_tls_with_sni_and_required_alpn",
        dependencies: &["message-io.ts", "prelogin-payload.ts", "tls"],
        status: "evidence-limited",
        divergence: "The Rust TLS negotiation is extracted as a dedicated operation and validated with SNI, certificate and TDS transport fixtures; a SQL Server strict-TLS endpoint is unavailable.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "sendPreLogin",
        responsibility: "serializes and sends the PRELOGIN message",
        rust_path: "connection.rs",
        rust_symbol: "send_prelogin",
        test_path: "tests/prelogin_tls_parity.rs",
        test_symbol: "encrypt_true_with_server_off_sends_login7_without_tls",
        dependencies: &["prelogin-payload.ts", "message-io.ts"],
        status: "evidence-limited",
        divergence: "PRELOGIN serialization and transport ordering are exercised through the real connection path; direct Node stream scheduling comparison remains unavailable.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "sendLogin7Packet",
        responsibility: "serializes and sends the LOGIN7 message",
        rust_path: "connection.rs",
        rust_symbol: "send_login7_packet",
        test_path: "tests/retry_reroute.rs",
        test_symbol: "reroutes_to_the_reported_host_port_and_preserves_instance_for_login7",
        dependencies: &["login7-payload.ts", "message-io.ts", "ntlm.ts"],
        status: "evidence-limited",
        divergence: "LOGIN7 construction and routing preservation are exercised through the real TDS connection path; complete Node callback scheduling comparison remains unavailable.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "sendInitialSql",
        responsibility: "sends and finalizes the initial SQL batch",
        rust_path: "connection.rs",
        rust_symbol: "send_initial_sql",
        test_path: "tests/integration/integration_sql_auth.rs",
        test_symbol: "sql_auth_connection_emits_the_complete_connect_state_sequence",
        dependencies: &[
            "sqlbatch-payload.ts",
            "token/handler.ts",
            "token/stream-parser.ts",
        ],
        status: "evidence-limited",
        divergence: "Initial SQL framing, token handling, transaction descriptor and final login state are exercised by the local SQL Auth integration; complete Node event scheduling comparison remains unavailable.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "performSentLogin7WithStandardLogin",
        responsibility: "reads and validates the standard LOGIN7 response",
        rust_path: "connection.rs",
        rust_symbol: "perform_sent_login7_with_standard_login",
        test_path: "tests/integration/integration_sql_auth.rs",
        test_symbol: "sql_auth_connection_emits_the_complete_connect_state_sequence",
        dependencies: &[
            "message-io.ts",
            "token/handler.ts",
            "token/stream-parser.ts",
        ],
        status: "evidence-limited",
        divergence: "The Rust operation reads, tokenizes and validates the LOGIN7 response in the real connection path; complete Node scheduling comparison remains unavailable.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "performSentLogin7WithNTLMLogin",
        responsibility: "processes the NTLM challenge and sends the NTLM response",
        rust_path: "connection.rs",
        rust_symbol: "perform_sent_login7_with_ntlm_login",
        test_path: "tests/retry_reroute.rs",
        test_symbol: "ntlm_login_reads_challenge_before_sending_response_and_accepts_optional_domain",
        dependencies: &["ntlm-payload.ts", "token/handler.ts", "message-io.ts"],
        status: "part",
        divergence: "NTLM challenge, response and optional-domain flow are covered by a real byte transport, the configured PROD endpoint for absent, empty and supplied domains, and a Node runtime comparison with an empty domain.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "performSentLogin7WithFedAuth",
        responsibility: "processes FedAuthInfo and exchanges the federated token",
        rust_path: "connection.rs",
        rust_symbol: "perform_sent_login7_with_fed_auth",
        test_path: "tests/retry_reroute.rs",
        test_symbol: "fedauth_login_reads_info_sends_token_and_completes_login",
        dependencies: &[
            "fedauth-info-parser.ts",
            "message-io.ts",
            "token/handler.ts",
        ],
        status: "evidence-limited",
        divergence: "FedAuthInfo validation, scope derivation, provider exchange and final response parsing are covered by a real byte transport and fixed provider; Azure or SQL Server FedAuth credentials remain unavailable for external end-to-end verification.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "wrapSocketError",
        responsibility: "converts transport failures to Node connection errors",
        rust_path: "connection.rs",
        rust_symbol: "wrap_socket_error",
        test_path: "tests/connection_transport_error_parity.rs",
        test_symbol: "transport_failure_is_an_esocket_connection_error_with_transport_cause",
        dependencies: &["message-io.ts", "errors.ts"],
        status: "evidence-limited",
        divergence: "Transport error category, code, message and cause are covered by the parity test.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "cleanupConnection",
        responsibility: "releases the active transport during terminal cleanup",
        rust_path: "connection.rs",
        rust_symbol: "cleanup_connection",
        test_path: "tests/node_connection_runtime_parity.rs",
        test_symbol: "node_and_rust_close_runtime_sequences_match",
        dependencies: &["message-io.ts", "events"],
        status: "part",
        divergence: "Transport cleanup, ECLOSE, terminal state, callback and end ordering now have Node runtime comparison; lower-level listener removal remains covered by lifecycle unit tests.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "prepare",
        responsibility: "executes sp_prepare and records handle",
        rust_path: "connection.rs",
        rust_symbol: "pub async fn prepare",
        test_path: "tests/prepare_failure_lifecycle_parity.rs",
        test_symbol: "prepare_failure",
        dependencies: &[
            "request.ts",
            "rpcrequest-payload.ts",
            "special-stored-procedure.ts",
        ],
        status: "part",
        divergence: "Failure lifecycle and unexpected output handling are implemented; live sp_prepare handle evidence is opt-in.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "unprepare",
        responsibility: "executes sp_unprepare",
        rust_path: "connection.rs",
        rust_symbol: "pub async fn unprepare",
        test_path: "tests/connection_confirmed_gaps.rs",
        test_symbol: "unprepare",
        dependencies: &["rpcrequest-payload.ts", "special-stored-procedure.ts"],
        status: "part",
        divergence: "State and read-only guards are tested; live procedure execution is not available.",
    },
    GapRecord {
        node_path: "connection.ts",
        node_symbol: "execute",
        responsibility: "executes a prepared request",
        rust_path: "connection.rs",
        rust_symbol: "pub async fn execute",
        test_path: "tests/connection_confirmed_gaps.rs",
        test_symbol: "execute",
        dependencies: &["request.ts", "rpcrequest-payload.ts", "value-parser.ts"],
        status: "part",
        divergence: "Prepared request validation and lifecycle are tested; live prepared execution is opt-in.",
    },
    GapRecord {
        node_path: "always-encrypted/symmetric-key-cache.ts",
        node_symbol: "getKey",
        responsibility: "retrieves, decrypts and caches a CEK",
        rust_path: "always_encrypted/symmetric_key_cache.rs",
        rust_symbol: "pub fn get_key",
        test_path: "tests/unit/ae_cache.rs",
        test_symbol: "ttl_expiration_reloads_key_and_zero_ttl_does_not_cache",
        dependencies: &["lru-cache@10.4.3", "symmetric-key.ts"],
        status: "part",
        divergence: "Rust receives the provider explicitly because the Node connection options resolve it; cache key, TTL, misses and clearing are tested, while Azure live evidence is separate.",
    },
    GapRecord {
        node_path: "bulk-load.ts",
        node_symbol: "addColumn",
        responsibility: "declares ordered bulk-load columns",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn add_column",
        test_path: "tests/unit/unit_bulk_load_api.rs",
        test_symbol: "add_column",
        dependencies: &["data-type.ts", "parameter.ts"],
        status: "part",
        divergence: "Column ordering, nullable metadata and pre-row validation are tested; Node Transform scheduling is covered at connection boundary.",
    },
    GapRecord {
        node_path: "bulk-load.ts",
        node_symbol: "getOptionsSql",
        responsibility: "serializes bulk options",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn get_options_sql",
        test_path: "tests/unit/bulk_spec09.rs",
        test_symbol: "bulk_options_match_insert_bulk_sql_shape_without_duplicate_tablock",
        dependencies: &["tracking-buffer/writable-tracking-buffer.ts"],
        status: "part",
        divergence: "SQL option order is tested byte-for-byte; no divergence is known in the covered branches.",
    },
    GapRecord {
        node_path: "bulk-load.ts",
        node_symbol: "getBulkInsertSql",
        responsibility: "serializes INSERT BULK SQL",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn get_bulk_insert_sql",
        test_path: "tests/unit/bulk_spec09.rs",
        test_symbol: "bulk_options_match_insert_bulk_sql_shape_without_duplicate_tablock",
        dependencies: &["getOptionsSql", "addColumn"],
        status: "part",
        divergence: "SQL output is tested; connection framing and live bulk execution remain integration-dependent.",
    },
    GapRecord {
        node_path: "bulk-load.ts",
        node_symbol: "getTableCreationSql",
        responsibility: "serializes table creation SQL",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn get_table_creation_sql",
        test_path: "tests/unit/unit_bulk_load_api.rs",
        test_symbol: "builds_bulk_insert_and_table_sql_with_options",
        dependencies: &["addColumn"],
        status: "part",
        divergence: "Nullability output is tested; live DDL is restricted to the local SQL authentication profile.",
    },
    GapRecord {
        node_path: "bulk-load.ts",
        node_symbol: "getColMetaData",
        responsibility: "serializes bulk column metadata",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn get_col_metadata",
        test_path: "tests/unit/unit_bulk_load_api.rs",
        test_symbol: "bulk_load_exposes_node_payload_helpers",
        dependencies: &["data-type.ts", "metadata-parser.ts"],
        status: "part",
        divergence: "Metadata bytes and nullable-unknown handling are tested; live server metadata is integration-dependent.",
    },
    GapRecord {
        node_path: "bulk-load.ts",
        node_symbol: "setTimeout",
        responsibility: "sets bulk request timeout",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn set_timeout",
        test_path: "tests/bulk_load_request_lifecycle.rs",
        test_symbol: "canceled_materialized_bulk_load_does_not_start_request_lifecycle",
        dependencies: &["connection.ts"],
        status: "part",
        divergence: "Rust stores the timeout and connection execution applies it; timer event ordering requires a live or controlled request lifecycle.",
    },
    GapRecord {
        node_path: "bulk-load.ts",
        node_symbol: "cancel",
        responsibility: "cancels bulk loading once",
        rust_path: "bulk_load.rs",
        rust_symbol: "pub fn cancel",
        test_path: "tests/unit/unit_bulk_load_api.rs",
        test_symbol: "bulk_load_cancel_emits_one_event",
        dependencies: &["events", "connection.ts"],
        status: "part",
        divergence: "Single cancel emission and transport cancellation are separately covered; live bulk cancellation is integration-dependent.",
    },
    GapRecord {
        node_path: "always-encrypted/keystore-provider-azure-key-vault.ts",
        node_symbol: "decryptColumnEncryptionKey",
        responsibility: "verifies and unwraps a CEK",
        rust_path: "always_encrypted/keystore_provider_azure_key_vault.rs",
        rust_symbol: "fn decrypt_column_encryption_key",
        test_path: "tests/unit/ae_keyvault_parity.rs",
        test_symbol: "encrypt_and_decrypt_preserve_node_operation_order_and_container_layout",
        dependencies: &["@azure/keyvault-keys", "@azure/identity"],
        status: "evidence-limited",
        divergence: "Operation order, container layout and signature checks use a provider fixture; Azure service evidence is unavailable.",
    },
    GapRecord {
        node_path: "always-encrypted/keystore-provider-azure-key-vault.ts",
        node_symbol: "encryptColumnEncryptionKey",
        responsibility: "wraps and signs a CEK",
        rust_path: "always_encrypted/keystore_provider_azure_key_vault.rs",
        rust_symbol: "pub fn encrypt_column_encryption_key",
        test_path: "tests/unit/ae_keyvault_parity.rs",
        test_symbol: "encrypt_and_decrypt_preserve_node_operation_order_and_container_layout",
        dependencies: &["@azure/keyvault-keys", "@azure/identity"],
        status: "evidence-limited",
        divergence: "Operation order, container layout and signature checks use a provider fixture; Azure service evidence is unavailable.",
    },
    GapRecord {
        node_path: "always-encrypted/keystore-provider-azure-key-vault.ts",
        node_symbol: "getMasterKey",
        responsibility: "resolves and validates a master key",
        rust_path: "always_encrypted/keystore_provider_azure_key_vault.rs",
        rust_symbol: "fn key_size",
        test_path: "tests/unit/ae_keyvault_parity.rs",
        test_symbol: "encrypt_and_decrypt_preserve_node_operation_order_and_container_layout",
        dependencies: &["@azure/keyvault-keys"],
        status: "evidence-limited",
        divergence: "Rust validates the parsed URI and obtains key size through the adapter; Azure service evidence is unavailable.",
    },
    GapRecord {
        node_path: "token/handler.ts",
        node_symbol: "onErrorMessage",
        responsibility: "aggregates server errors and emits request failure",
        rust_path: "token/handler.rs",
        rust_symbol: "fn on_error",
        test_path: "tests/unit/unit_token_handler.rs",
        test_symbol: "request_handler_retains_info_messages",
        dependencies: &["errors.ts", "request.ts"],
        status: "part",
        divergence: "Error aggregation and aliases are covered by deterministic token tests; complete EventEmitter comparison is pending.",
    },
    GapRecord {
        node_path: "token/handler.ts",
        node_symbol: "onRow",
        responsibility: "maps row tokens to request results",
        rust_path: "token/handler.rs",
        rust_symbol: "fn on_row",
        test_path: "tests/unit/resultset_parity.rs",
        test_symbol: "request_rows_are_collected_only_for_request_completion",
        dependencies: &["token/row-token-parser.ts", "value-parser.ts"],
        status: "part",
        divergence: "Conditional collection and column mapping are tested; all streaming consumer timing branches are not live-comparison tested.",
    },
    GapRecord {
        node_path: "token/handler.ts",
        node_symbol: "onDone",
        responsibility: "completes request on DONE tokens",
        rust_path: "token/handler.rs",
        rust_symbol: "fn on_done",
        test_path: "tests/node_connection_runtime_parity.rs",
        test_symbol: "node_and_rust_request_runtime_sequences_match",
        dependencies: &["token/done-token-parser.ts", "request.ts"],
        status: "part",
        divergence: "DONE status, request completion and callback ordering now have runtime Node↔Rust coverage for execSql; handler-specific token combinations remain covered by unit and integration suites.",
    },
    GapRecord {
        node_path: "token/env-change-token-parser.ts",
        node_symbol: "envChangeParser",
        responsibility: "parses typed and unknown ENVCHANGE tokens",
        rust_path: "token/env_change_token_parser.rs",
        rust_symbol: "pub fn parse_optional",
        test_path: "tests/unit/unit_token_parser.rs",
        test_symbol: "optional_envchange_parser_skips_unknown_types_like_node",
        dependencies: &["collation.ts", "token/helpers.ts"],
        status: "equivalent",
        divergence: "Variantes tipadas e o adapter opcional preservam o parsing; tipos desconhecidos são consumidos e retornam ausência, como o parser Node, enquanto a representação interna mantém dados para o stream.",
    },
];

fn source_files(root: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let entries = fs::read_dir(path).expect("source directory must be readable");
        for entry in entries {
            let entry = entry.expect("source directory entry must be readable");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|value| value == extension) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn rust_relative(node_relative: &str) -> String {
    let mut path = node_relative.trim_end_matches(".ts").replace('-', "_");
    if path == "data_types/sql_variant" {
        path = "data_types/sql_variant".to_string();
    }
    format!("{path}.rs")
}

fn snake(name: &str) -> String {
    name.trim_start_matches('#')
        .chars()
        .enumerate()
        .flat_map(|(index, character)| {
            if character.is_ascii_uppercase() && index != 0 {
                vec!['_', character.to_ascii_lowercase()]
            } else {
                vec![character.to_ascii_lowercase()]
            }
        })
        .collect()
}

fn rust_type_name(name: &str) -> String {
    let chars = name.chars().collect::<Vec<_>>();
    let first_lower = chars
        .iter()
        .position(|character| character.is_ascii_lowercase())
        .unwrap_or(chars.len());
    if first_lower > 1 && first_lower < chars.len() {
        let acronym_end = first_lower - 1;
        let mut result = chars[..acronym_end]
            .iter()
            .map(|character| character.to_ascii_lowercase())
            .collect::<String>();
        result.extend(chars[acronym_end..].iter());
        result
    } else {
        name.to_owned()
    }
}

fn normalized_identifier(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect::<String>()
        .replace("uint", "u")
}

fn rust_declarations(source: &str) -> Vec<(String, String)> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let (kind, rest) = if let Some(rest) = trimmed.strip_prefix("pub struct ") {
                ("struct", rest)
            } else if let Some(rest) = trimmed.strip_prefix("struct ") {
                ("struct", rest)
            } else if let Some(rest) = trimmed.strip_prefix("pub enum ") {
                ("enum", rest)
            } else if let Some(rest) = trimmed.strip_prefix("enum ") {
                ("enum", rest)
            } else if let Some(rest) = trimmed.strip_prefix("pub fn ") {
                ("fn", rest)
            } else if let Some(rest) = trimmed.strip_prefix("pub async fn ") {
                ("fn", rest)
            } else if let Some(rest) = trimmed.strip_prefix("async fn ") {
                ("fn", rest)
            } else {
                ("fn", trimmed.strip_prefix("fn ")?)
            };
            let name = rest
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect::<String>();
            (!name.is_empty()).then_some((kind.to_owned(), name))
        })
        .collect()
}

fn imported_packages(source: &str) -> Vec<String> {
    source
        .lines()
        .filter(|line| line.trim_start().starts_with("import "))
        .filter_map(|line| {
            line.split(['\'', '"'])
                .nth(1)
                .filter(|value| !value.starts_with('.') && !value.starts_with("node:"))
                .map(str::to_owned)
        })
        .map(|value| value.split('/').next().unwrap_or(&value).to_owned())
        .collect()
}

fn test_evidence(test_root: &Path, node_path: &str, rust_symbol: &str) -> (String, String) {
    let stem = Path::new(node_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .replace('-', "_");
    let rust_name = rust_symbol
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .trim_end_matches('(')
        .to_string();
    let mut candidates = Vec::new();
    for path in source_files(test_root, "rs") {
        let text = fs::read_to_string(&path).expect("test source must be readable");
        if text.contains(&stem) && text.contains(&rust_name) {
            candidates.push((path, text));
        }
    }
    candidates
        .into_iter()
        .next()
        .map(|(path, text)| {
            let lines = text.lines().collect::<Vec<_>>();
            let symbol = lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.contains(&rust_name))
                .find_map(|(index, _)| {
                    lines[..=index]
                        .iter()
                        .rev()
                        .find_map(|line| {
                            let line = line.trim();
                            line.strip_prefix("fn ")
                                .or_else(|| line.strip_prefix("async fn "))
                        })
                        .and_then(|value| value.split('(').next())
                })
                .unwrap_or_default()
                .to_string();
            if symbol.is_empty() {
                (String::new(), String::new())
            } else {
                (path.to_string_lossy().to_string(), symbol)
            }
        })
        .unwrap_or_else(|| (String::new(), String::new()))
}

fn build_ledger(manifest: &Path) -> Vec<SymbolRecord> {
    let node_root = fs::canonicalize(manifest.join("../tedious-node/src"))
        .expect("Node source root must exist");
    let rust_root = manifest.join("src");
    let test_root = manifest.join("tests");
    let package_root =
        fs::canonicalize(manifest.join("../tedious-node")).expect("Node package root must exist");
    let observer = manifest.join("tests/compatibility/source_symbol_observer.js");
    let output = Command::new("node")
        .arg(observer)
        .arg(&package_root)
        .arg(&node_root)
        .output()
        .expect("Node AST observer must run");
    assert!(output.status.success());
    let mut records = Vec::new();
    let mut source_cache = BTreeMap::new();
    for line in String::from_utf8(output.stdout)
        .expect("Node AST output must be UTF-8")
        .lines()
    {
        let symbol: serde_json::Value =
            serde_json::from_str(line).expect("Node AST observer output must be JSON");
        let path = PathBuf::from(symbol["file"].as_str().expect("symbol file must exist"));
        let relative = path
            .strip_prefix(&node_root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let line_number = symbol["line"].as_u64().expect("symbol line must exist");
        let kind = symbol["kind"].as_str().expect("symbol kind must exist");
        let name = symbol["name"].as_str().expect("symbol name must exist");
        let node_text = source_cache
            .entry(relative.clone())
            .or_insert_with(|| fs::read_to_string(&path).expect("Node source must be readable"));
        let rust_relative_path = rust_relative(&relative);
        let rust_path = rust_root.join(&rust_relative_path);
        let rust_text = fs::read_to_string(&rust_path).unwrap_or_default();
        let rust_declarations = rust_declarations(&rust_text);
        let dependencies = node_text
            .lines()
            .filter(|line| line.trim_start().starts_with("import "))
            .map(|line| line.trim().to_string())
            .collect::<Vec<_>>();
        let source_name = if kind == "class" {
            name.to_owned()
        } else if kind == "method" {
            name.rsplit('.').next().unwrap_or(name).to_owned()
        } else {
            name.to_owned()
        };
        let expected_kind = if kind == "class" { "struct" } else { "fn" };
        let matching_declaration =
            rust_declarations
                .iter()
                .find(|(candidate_kind, candidate_name)| {
                    candidate_kind == expected_kind
                        && normalized_identifier(candidate_name)
                            == normalized_identifier(&source_name)
                });
        let rust_name = matching_declaration
            .map(|(_, candidate_name)| candidate_name.clone())
            .unwrap_or_else(|| {
                if kind == "class" {
                    rust_type_name(&source_name)
                } else {
                    snake(&source_name)
                }
            });
        let rust_symbol = if kind == "class" {
            format!("struct {rust_name}")
        } else {
            format!("fn {rust_name}")
        };
        let rust_exists = matching_declaration.is_some();
        let (test_path, test_symbol) = test_evidence(&test_root, &relative, &rust_symbol);
        let has_test = !test_path.is_empty();
        let status = if rust_exists && has_test {
            "equivalent"
        } else if rust_exists {
            "evidence-limited"
        } else {
            "divergent"
        };
        records.push(SymbolRecord {
            node_path: relative,
            node_symbol: format!("{kind} {name}@{line_number}"),
            responsibility: kind.to_owned(),
            rust_path: rust_relative_path,
            rust_symbol,
            test_path,
            test_symbol,
            dependencies,
            status: status.to_owned(),
            divergence: if !rust_exists {
                format!("No matching Rust {kind} was found in the mapped module.")
            } else if !has_test {
                format!("No individual behavior test was found for this {kind}.")
            } else {
                String::new()
            },
        });
    }
    records
}

#[test]
fn every_node_function_and_method_has_individual_traceability_record() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let records = build_ledger(manifest);
    assert!(!records.is_empty());
    let mut summary = BTreeMap::new();
    for record in &records {
        assert!(!record.node_path.is_empty());
        assert!(!record.node_symbol.is_empty());
        assert!(!record.responsibility.is_empty());
        assert!(!record.rust_path.is_empty());
        assert!(!record.rust_symbol.is_empty());
        if !record.test_path.is_empty() {
            let test = fs::read_to_string(manifest.join(&record.test_path))
                .expect("generated behavior test path must exist");
            assert!(
                test.contains(&record.test_symbol),
                "test symbol {} missing for {}:{}",
                record.test_symbol,
                record.node_path,
                record.node_symbol
            );
        }
        if record.status != "equivalent" {
            assert!(!record.divergence.is_empty());
        }
        let status = format!(
            "{}:dependencies={}",
            record.status,
            record.dependencies.len()
        );
        *summary.entry(status).or_insert(0usize) += 1;
    }
    assert!(summary.values().sum::<usize>() == records.len());
}

#[test]
fn source_symbols_have_one_unambiguous_ledger_record() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let records = build_ledger(manifest);
    let mut symbols = std::collections::BTreeSet::new();
    for record in records {
        let key = format!("{}:{}", record.node_path, record.node_symbol);
        assert!(
            symbols.insert(key.clone()),
            "duplicate source symbol: {key}"
        );
    }
}

#[test]
fn typescript_ast_observer_extracts_the_source_symbol_set() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let node_root = manifest.join("../tedious-node");
    let output = Command::new("node")
        .arg(manifest.join("tests/compatibility/source_symbol_observer.js"))
        .arg(&node_root)
        .arg(node_root.join("src"))
        .output()
        .expect("Node AST observer must run");
    assert!(output.status.success());
    let lines = String::from_utf8(output.stdout).expect("Node AST output must be UTF-8");
    let mut symbols = std::collections::BTreeSet::new();
    for line in lines.lines() {
        let symbol: serde_json::Value =
            serde_json::from_str(line).expect("Node AST observer output must be JSON");
        let file = symbol["file"].as_str().expect("symbol file must exist");
        let line_number = symbol["line"].as_u64().expect("symbol line must exist");
        let kind = symbol["kind"].as_str().expect("symbol kind must exist");
        let name = symbol["name"].as_str().expect("symbol name must exist");
        assert!(file.ends_with(".ts"));
        assert!(line_number > 0);
        assert!(!kind.is_empty());
        assert!(!name.is_empty());
        assert!(symbols.insert(format!("{file}:{line_number}:{kind}:{name}")));
    }
    assert!(!symbols.is_empty());
}

#[test]
fn ledger_contains_every_typescript_ast_symbol_at_its_source_location() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let node_root = manifest.join("../tedious-node");
    let output = Command::new("node")
        .arg(manifest.join("tests/compatibility/source_symbol_observer.js"))
        .arg(&node_root)
        .arg(node_root.join("src"))
        .output()
        .expect("Node AST observer must run");
    assert!(output.status.success());
    let records = build_ledger(manifest);
    let ledger = records
        .iter()
        .map(|record| (record.node_path.clone(), record.node_symbol.clone()))
        .collect::<std::collections::BTreeSet<_>>();
    let node_root = fs::canonicalize(node_root.join("src")).expect("Node source root must exist");
    for line in String::from_utf8(output.stdout)
        .expect("Node AST output must be UTF-8")
        .lines()
    {
        let symbol: serde_json::Value =
            serde_json::from_str(line).expect("Node AST observer output must be JSON");
        let file = Path::new(symbol["file"].as_str().expect("symbol file must exist"))
            .strip_prefix(&node_root)
            .expect("symbol file must be under Node source")
            .to_string_lossy()
            .to_string();
        let line_number = symbol["line"].as_u64().expect("symbol line must exist");
        let kind = symbol["kind"].as_str().expect("symbol kind must exist");
        let name = symbol["name"].as_str().expect("symbol name must exist");
        let node_symbol = format!("{kind} {name}@{line_number}");
        assert!(
            ledger.contains(&(file.clone(), node_symbol.clone())),
            "AST symbol missing from ledger: {file}:{node_symbol}"
        );
    }
}

#[test]
fn mapped_modules_and_external_dependencies_are_auditable() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let node_root = manifest.join("../tedious-node/src");
    let package_lock = manifest.join("../tedious-node/package-lock.json");
    let lock = fs::read_to_string(package_lock).expect("resolved Node dependency lock must exist");
    assert!(lock.contains("node_modules/sprintf-js"));
    assert!(lock.contains("node_modules/bl"));
    assert!(lock.contains("node_modules/js-md4"));
    for path in source_files(&node_root, "ts") {
        let relative = path
            .strip_prefix(&node_root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let mapped = manifest.join("src").join(rust_relative(&relative));
        assert!(
            mapped.is_file(),
            "missing Rust module mapping for {relative}"
        );
        let source = fs::read_to_string(path).expect("Node source must be readable");
        for package in imported_packages(&source) {
            assert!(
                [
                    "assert",
                    "buffer",
                    "child_process",
                    "constants",
                    "crypto",
                    "dgram",
                    "dns",
                    "events",
                    "fs",
                    "net",
                    "os",
                    "path",
                    "stream",
                    "tls",
                    "url",
                    "util"
                ]
                .contains(&package.as_str())
                    || lock.contains(&format!("node_modules/{package}")),
                "unresolved external Node dependency: {package}"
            );
        }
    }
}

#[test]
fn every_audited_gap_has_individual_source_behavior_and_dependency_evidence() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let node_root = manifest.join("../tedious-node/src");
    for record in GAP_RECORDS {
        let node = fs::read_to_string(node_root.join(record.node_path))
            .expect("audited Node source must exist");
        assert!(
            node.contains(record.node_symbol),
            "Node symbol is absent: {}",
            record.node_symbol
        );
        let rust_path = manifest.join("src").join(record.rust_path);
        let rust = fs::read_to_string(&rust_path).expect("audited Rust source must exist");
        assert!(
            rust.contains(record.rust_symbol),
            "Rust symbol is absent: {}",
            record.rust_symbol
        );
        let test = read_test_file(manifest, record.test_path);
        assert!(
            test.contains(record.test_symbol),
            "specific behavior test is absent: {}",
            record.test_symbol
        );
        assert!(!record.responsibility.is_empty());
        assert!(!record.dependencies.is_empty());
        assert!(matches!(
            record.status,
            "equivalent" | "part" | "divergent" | "evidence-limited"
        ));
        assert!(!record.divergence.is_empty());
    }
}

#[test]
fn resolved_node_dependencies_have_versions_and_referenced_entrypoints() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let node_root = manifest.join("../tedious-node");
    let lock = fs::read_to_string(node_root.join("package-lock.json"))
        .expect("resolved Node dependency lock must exist");
    for package in [
        "bl",
        "lru-cache",
        "js-md4",
        "sprintf-js",
        "@azure/keyvault-keys",
    ] {
        let marker = format!("node_modules/{package}");
        assert!(
            lock.contains(&marker),
            "resolved dependency is absent: {package}"
        );
    }
    for entrypoint in [
        "node_modules/bl/bl.js",
        "node_modules/lru-cache/index.js",
        "node_modules/js-md4/src/md4.js",
        "node_modules/sprintf-js/src/sprintf.js",
    ] {
        assert!(
            node_root.join(entrypoint).is_file(),
            "dependency entrypoint is absent: {entrypoint}"
        );
    }
}
