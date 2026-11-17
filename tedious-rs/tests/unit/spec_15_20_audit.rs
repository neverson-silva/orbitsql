use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

fn project_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn integrity_rows() -> Vec<(String, String)> {
    let content = fs::read_to_string(project_root().join("../specs/20-source-integrity-ledger.md"))
        .expect("source integrity ledger must exist");
    content
        .lines()
        .filter_map(|line| {
            let fields = line.split(char::from(96)).collect::<Vec<_>>();
            (fields.len() >= 4 && fields[1].starts_with("src/") && fields[3].len() == 64)
                .then(|| (fields[1].to_owned(), fields[3].to_owned()))
        })
        .collect()
}

#[test]
fn every_source_integrity_hash_matches_the_node_snapshot() {
    let rows = integrity_rows();
    assert!(!rows.is_empty());
    for (relative, expected) in rows {
        let bytes = fs::read(project_root().join("../tedious-node").join(&relative))
            .unwrap_or_else(|_| panic!("source file is missing: {relative}"));
        let actual = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(actual, expected, "source hash changed: {relative}");
    }
}

#[test]
fn dependency_audit_keeps_resolved_node_entrypoints_and_rust_adapters() {
    let node_root = project_root().join("../tedious-node");
    let package_lock =
        fs::read_to_string(node_root.join("package-lock.json")).expect("Node lock must exist");
    let lock_json: serde_json::Value =
        serde_json::from_str(&package_lock).expect("Node lock must be valid JSON");
    for (package, version) in [
        ("node_modules/bl", "6.1.6"),
        ("node_modules/lru-cache", "5.1.1"),
        ("node_modules/js-md4", "0.3.2"),
        ("node_modules/sprintf-js", "1.1.3"),
        ("node_modules/@azure/keyvault-keys", "4.10.2"),
        ("node_modules/@azure/identity", "4.13.1"),
        ("node_modules/iconv-lite", "0.7.2"),
    ] {
        let resolved = lock_json["packages"][package]["version"]
            .as_str()
            .unwrap_or_default();
        assert_eq!(
            resolved, version,
            "resolved dependency version changed: {package}"
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
            "dependency entrypoint is missing: {entrypoint}"
        );
    }
    for adapter in [
        "src/always_encrypted/keystore_provider_azure_key_vault.rs",
        "src/always_encrypted/symmetric_key_cache.rs",
        "src/azure_auth.rs",
        "src/ntlm.rs",
        "src/value_codec.rs",
        "src/value_parser.rs",
    ] {
        assert!(
            project_root().join(adapter).is_file(),
            "Rust adapter is missing: {adapter}"
        );
    }
    let cargo_lock =
        fs::read_to_string(project_root().join("Cargo.lock")).expect("Cargo lock must exist");
    for (crate_name, version) in [
        ("encoding_rs", "0.8.35"),
        ("yore", "2.1.1"),
        ("md4", "0.11.0"),
        ("reqwest", "0.12.28"),
        ("rustls", "0.23.43"),
        ("tokio", "1.53.1"),
    ] {
        let marker = format!("name = {}{}{}", '"', crate_name, '"');
        let position = cargo_lock
            .find(&marker)
            .unwrap_or_else(|| panic!("crate is not locked: {crate_name}"));
        let section = &cargo_lock[position..cargo_lock.len().min(position + 160)];
        assert!(
            section.contains(&format!("version = {}{}{}", '"', version, '"')),
            "resolved crate version changed: {crate_name}"
        );
    }
}

#[test]
fn source_integrity_and_dependency_specs_are_currently_referenced() {
    let root = project_root().join("../specs");
    let spec15 =
        fs::read_to_string(root.join("15-dependency-audit.md")).expect("Spec 15 must exist");
    let evidence = fs::read_to_string(root.join("dependency-evidence/15-observations.md"))
        .expect("dependency evidence must exist");
    for value in [
        "iconv-lite",
        "js-md4",
        "lru-cache",
        "@azure/identity",
        "@azure/keyvault-keys",
        "reqwest",
        "rustls",
        "encoding_rs",
        "yore",
    ] {
        assert!(spec15.contains(value), "Spec 15 omits dependency: {value}");
        assert!(
            evidence.contains(value),
            "dependency evidence omits dependency: {value}"
        );
    }
}

#[test]
fn function_contract_and_behavioral_diff_specs_have_executable_requirements() {
    let specs_root = project_root().join("../specs");
    let contract = fs::read_to_string(specs_root.join("18-function-port-contract.md"))
        .expect("Spec 18 must exist");
    for field in [
        "source:",
        "symbol:",
        "rust_target:",
        "inputs_defaults:",
        "state_reads_writes:",
        "external_calls:",
        "success_behavior:",
        "error_behavior:",
        "events_callbacks_order:",
        "bytes_or_values:",
        "tests:",
        "divergence:",
    ] {
        assert!(
            contract.contains(field),
            "Spec 18 omits contract field: {field}"
        );
    }
    let contract_rows = contract
        .lines()
        .filter(|line| line.starts_with("| `src/") && line.ends_with("|"))
        .count();
    assert!(contract_rows > 0);
    assert!(contract.contains("source_symbol_observer.js"));
    assert!(contract.contains("spec_05_source_symbol_ledger.rs"));

    let diff = fs::read_to_string(specs_root.join("19-behavioral-diff-and-fixtures.md"))
        .expect("Spec 19 must exist");
    for requirement in [
        "bytes enviados/recebidos",
        "tokens",
        "eventos",
        "erro",
        "estado final",
        "chunks TCP",
    ] {
        assert!(
            diff.contains(requirement),
            "Spec 19 omits requirement: {requirement}"
        );
    }
    for directory in [
        "fixtures/packets",
        "fixtures/payloads",
        "fixtures/tokens",
        "fixtures/types",
        "fixtures/auth",
    ] {
        assert!(project_root().join("tests").join(directory).is_dir());
    }
    assert!(
        project_root()
            .join("tests/compatibility/source_symbol_observer.js")
            .is_file()
    );
}
