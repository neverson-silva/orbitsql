use std::fs;
use std::path::PathBuf;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repository_root() -> PathBuf {
    project_root().join("..").to_path_buf()
}

fn read_spec(name: &str) -> String {
    fs::read_to_string(repository_root().join("specs").join(name))
        .unwrap_or_else(|error| panic!("specification {name} is unreadable: {error}"))
}

fn assert_files_exist(paths: &[&str]) {
    for path in paths {
        assert!(
            project_root().join(path).is_file(),
            "required port artifact is missing: {path}"
        );
    }
}

#[test]
fn integration_harness_has_separate_mutating_and_ntlm_profiles() {
    let spec = read_spec("27-integration-harness-contract.md");
    for required in [
        "sql_auth_mutating",
        "ntlm_prod_read_only",
        "localhost",
        "sa",
        "DevStr0ng\\@Pass",
        "tedious_rs",
        "TEST_DB_HOST",
        "VPN_USER",
        "VPN_PWD",
        "TEST_DB_NAME",
        "VPN_DOMAIN",
        "metadata",
        "CRUD",
        "TVP",
        "bulk",
        "SELECT 1",
    ] {
        assert!(spec.contains(required), "Spec 27 omits {required}");
    }

    assert_files_exist(&[
        "tests/common/mod.rs",
        "tests/integration/sql_auth_harness.rs",
        "tests/integration/ntlm_read_only_harness.rs",
        "tests/integration/integration_sql_auth.rs",
        "tests/integration/bulk_load.rs",
        "tests/integration/tvp.rs",
    ]);

    let common = fs::read_to_string(project_root().join("tests/common/mod.rs")).unwrap();
    for required in [
        "ConnectionConfig::sql(\"localhost\", \"sa\", \"DevStr0ng@Pass\")",
        "TEST_DB_HOST",
        "VPN_USER",
        "VPN_PWD",
        "TEST_DB_NAME",
        "VPN_DOMAIN",
        "read_only = true",
        "ntlm_domain_variants",
        "with_sql_connection_cleanup_with_config",
    ] {
        assert!(
            common.contains(required),
            "integration loader omits {required}"
        );
    }

    let ntlm =
        fs::read_to_string(project_root().join("tests/integration/ntlm_read_only_harness.rs"))
            .unwrap();
    assert!(ntlm.contains("must reject mutating SQL before network IO"));
    assert!(ntlm.contains("ntlm_prod_profile_is_read_only_for_all_domain_states"));
    assert!(ntlm.contains("VPN_DOMAIN"));
}

#[test]
fn dependency_contract_has_resolved_crates_and_source_adapters() {
    let spec = read_spec("28-dependency-adapter-contract.md");
    for required in [
        "net`/`dns",
        "tls",
        "stream`/`events",
        "crypto",
        "js-md4",
        "iconv-lite",
        "@js-joda/core",
        "Azure SDKs",
        "lru-cache",
        "fixtures do Node",
    ] {
        assert!(spec.contains(required), "Spec 28 omits {required}");
    }

    let manifest = fs::read_to_string(project_root().join("Cargo.toml")).unwrap();
    for dependency in [
        "encoding_rs",
        "yore",
        "aes",
        "hmac",
        "sha2",
        "md4",
        "native-tls",
        "rustls",
        "reqwest",
        "tokio",
    ] {
        assert!(
            manifest.contains(dependency),
            "Rust adapter dependency is absent: {dependency}"
        );
    }

    assert_files_exist(&[
        "src/connector.rs",
        "src/transport.rs",
        "src/ntlm.rs",
        "src/value_codec.rs",
        "src/azure_auth.rs",
        "src/always_encrypted/keystore_provider_azure_key_vault.rs",
        "src/always_encrypted/symmetric_key_cache.rs",
        "tests/unit/unit_ntlm.rs",
        "tests/unit/unit_tls.rs",
        "tests/unit/unit_encoding_parity.rs",
        "tests/unit/unit_azure_auth.rs",
        "tests/unit/unit_azure_key_vault.rs",
        "tests/unit/ae_cache.rs",
    ]);
}

#[test]
fn ast_contract_and_implemented_evidence_are_connected_to_tests() {
    let ast = read_spec("29-ast-construction-contracts.md");
    let evidence = read_spec("30-implemented-port-evidence.md");
    for required in [
        "Contrato gerado do snapshot",
        "assinatura",
        "corpo ainda deve ser traduzido linha a linha",
        "Teste mínimo",
    ] {
        assert!(ast.contains(required), "Spec 29 omits {required}");
    }
    for required in [
        "Implementação com evidência local",
        "Teste externo",
        "Resultado",
        "Implementação existente sem comprovação de integração",
        "Uma unidade só muda para `verified`",
    ] {
        assert!(evidence.contains(required), "Spec 30 omits {required}");
    }

    assert_files_exist(&[
        "tests/compatibility/source_symbol_observer.js",
        "tests/unit/spec_05_source_symbol_ledger.rs",
        "tests/unit/spec_15_20_audit.rs",
        "tests/unit/spec_21_26_audit.rs",
        "tests/compatibility/node_rust_gate.rs",
        "tests/unit/unit_primitives.rs",
        "tests/unit/unit_connection.rs",
        "tests/unit/unit_token_parser.rs",
        "tests/unit/unit_data_types.rs",
    ]);
    assert!(evidence.contains("858 testes unitários"));
    assert!(evidence.contains("aprovou 30 cenários e não teve falhas"));
}

#[test]
fn incremental_message_contract_exposes_real_public_events_and_backpressure() {
    let spec = read_spec("31-message-stream-incremental-gap.md");
    for required in [
        "feed_incremental",
        "resume_incremental",
        "MessageStreamEvent::MessageStarted",
        "Data",
        "MessageEnded",
        "feed_incremental_with_sink",
        "MessageStreamControl::Pending",
        "resume_incremental_with_sink",
        "primeiro pacote",
        "payload vazio",
    ] {
        assert!(spec.contains(required), "Spec 31 omits {required}");
    }

    assert_files_exist(&[
        "src/message.rs",
        "src/incoming_message_stream.rs",
        "tests/unit/incremental_backpressure.rs",
        "tests/unit/incoming_stream_pending.rs",
        "tests/unit/incoming_stream_state_parity.rs",
        "tests/unit/stream_backpressure_drain_parity.rs",
        "tests/unit/incoming_stream_materialization_parity.rs",
        "tests/compatibility/node_incoming_stream_runtime_parity.js",
    ]);

    let message = fs::read_to_string(project_root().join("src/message.rs")).unwrap();
    let incoming =
        fs::read_to_string(project_root().join("src/incoming_message_stream.rs")).unwrap();
    for required in [
        "pub enum MessageStreamEvent",
        "MessageStarted",
        "Data",
        "MessageEnded",
        "pub enum MessageStreamControl",
        "Pending",
    ] {
        assert!(
            message.contains(required),
            "message stream type omits {required}"
        );
    }
    for required in [
        "pub fn feed_incremental",
        "pub fn resume_incremental",
        "pub fn feed_incremental_with_sink",
        "pub fn resume_incremental_with_sink",
    ] {
        assert!(
            incoming.contains(required),
            "public incremental API omits {required}"
        );
    }
}
