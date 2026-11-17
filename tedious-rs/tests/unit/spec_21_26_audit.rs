use std::fs;
use std::path::Path;

fn project_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read_spec(name: &str) -> String {
    fs::read_to_string(project_root().join("../specs").join(name))
        .unwrap_or_else(|_| panic!("specification is missing: {name}"))
}

#[test]
fn construction_index_and_symbol_contract_define_complete_traceability() {
    let index = read_spec("21-construction-index.md");
    for section in [
        "## Regra de leitura",
        "## Ordem de construção",
        "## Artefatos por unidade",
        "## Definição de pronto",
    ] {
        assert!(index.contains(section), "Spec 21 omits section: {section}");
    }
    for artifact in [
        "módulo Rust equivalente",
        "ficha de cada símbolo",
        "fixtures determinísticas",
        "testes unitários",
        "testes de integração aplicáveis",
        "decisão de crate externo",
        "registro de divergência",
    ] {
        assert!(
            index.contains(artifact),
            "Spec 21 omits artifact: {artifact}"
        );
    }

    let contract = read_spec("22-symbol-construction-contract.md");
    for field in [
        "source_file:",
        "source_range:",
        "symbol:",
        "rust_module:",
        "rust_symbol:",
        "inputs:",
        "outputs:",
        "state:",
        "calls:",
        "branches:",
        "errors:",
        "events:",
        "resources:",
        "fixture:",
        "test:",
        "status:",
        "divergence:",
    ] {
        assert!(contract.contains(field), "Spec 22 omits field: {field}");
    }
    assert!(contract.contains("bytes mínimos"));
    assert!(contract.contains("Overloads do TypeScript"));
    assert!(project_root().join("src").is_dir());
    assert!(project_root().join("tests").is_dir());
}

#[test]
fn packet_prelogin_token_and_type_specs_have_concrete_contracts() {
    let packet = read_spec("23-packet-prelogin-contract.md");
    for value in [
        "HEADER_LENGTH = 8",
        "Length, incluindo header",
        "PacketID",
        "PRELOGIN option table",
        "offset/length fora do buffer",
        "Fixtures mínimas",
    ] {
        assert!(packet.contains(value), "Spec 23 omits requirement: {value}");
    }
    for source in [
        "src/packet.rs",
        "src/prelogin_payload.rs",
        "tests/unit/packet_transport_robustness.rs",
        "tests/unit/unit_prelogin_payload.rs",
    ] {
        assert!(
            project_root().join(source).is_file(),
            "Spec 23 target is missing: {source}"
        );
    }

    let tokens = read_spec("24-token-contract.md");
    for value in [
        "Tokens desconhecidos",
        "NotEnoughData",
        "colMetadata",
        "Handler matrix",
        "UnexpectedTokenError",
        "attention",
    ] {
        assert!(tokens.contains(value), "Spec 24 omits requirement: {value}");
    }
    for source in [
        "src/token/parser.rs",
        "src/token/handler.rs",
        "src/token/stream.rs",
    ] {
        assert!(
            project_root().join(source).is_file(),
            "Spec 24 target is missing: {source}"
        );
    }

    let types = read_spec("25-data-type-construction-matrix.md");
    for family in [
        "integer",
        "bit",
        "float",
        "money",
        "exact",
        "char",
        "nchar",
        "binary",
        "temporal",
        "identity",
        "structured",
    ] {
        assert!(types.contains(family), "Spec 25 omits family: {family}");
    }
    for requirement in [
        "declaration/type info",
        "parameter length/data",
        "validate",
        "null/invalid input",
        "not implemented",
        "unsupported",
    ] {
        assert!(
            types.contains(requirement),
            "Spec 25 omits requirement: {requirement}"
        );
    }
    assert!(project_root().join("src/data_types").is_dir());
    assert!(
        project_root()
            .join("tests/unit/unit_data_types.rs")
            .is_file()
    );
}

#[test]
fn connection_spec_maps_state_methods_timers_and_error_gates() {
    let connection = read_spec("26-connection-construction-contract.md");
    for field in [
        "fedAuthRequired",
        "inTransaction",
        "transaction depth",
        "retry count",
        "routing data",
        "MessageIO",
        "database collation",
        "cancel/request timers",
    ] {
        assert!(
            connection.contains(field),
            "Spec 26 omits state field: {field}"
        );
    }
    for method in [
        "connect",
        "close",
        "execSql",
        "execSqlBatch",
        "newBulkLoad",
        "execBulkLoad",
        "prepare",
        "unprepare",
        "execute",
        "callProcedure",
        "beginTransaction",
        "commitTransaction",
        "rollbackTransaction",
        "saveTransaction",
        "transaction",
        "cancel",
        "reset",
        "initialiseConnection",
        "performReRouting",
        "performTransientFailureRetry",
        "performSentLogin7WithNTLMLogin",
    ] {
        assert!(
            connection.contains(method),
            "Spec 26 omits method: {method}"
        );
    }
    for default in [
        "connect 15s",
        "request 15s",
        "cancel 5s",
        "retry interval 500ms",
    ] {
        assert!(
            connection.contains(default),
            "Spec 26 omits default: {default}"
        );
    }
    for error in [
        "prelogin inválido",
        "login falho",
        "cancel sem ACK",
        "request concorrente",
        "retry transitório",
        "rerouting",
    ] {
        assert!(
            connection.contains(error),
            "Spec 26 omits error scenario: {error}"
        );
    }
    assert!(project_root().join("src/connection.rs").is_file());
    assert!(
        project_root()
            .join("tests/unit/unit_connection.rs")
            .is_file()
    );
    assert!(project_root().join("tests/compatibility.rs").is_file());
}
