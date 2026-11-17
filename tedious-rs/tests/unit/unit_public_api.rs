use tedious_rs::library::NAME;
use tedious_rs::special_stored_procedure::SpecialStoredProcedure;
use tedious_rs::{CONNECTION_EVENT_NAMES, ConnectionEventName};

#[test]
fn exposes_node_library_identity_and_special_procedure_ids() {
    assert_eq!(NAME, "Tedious");
    assert_eq!(tedious_rs::LIBRARY.name, "Tedious");
    assert_eq!(tedious_rs::library::library().name, "Tedious");
    assert_eq!(tedious_rs::library().name, "Tedious");
    assert_eq!(SpecialStoredProcedure::SpExecuteSql.id(), 10);
    assert_eq!(SpecialStoredProcedure::SpUnprepare.id(), 15);
}

#[test]
fn exposes_the_compatibility_facade_symbols() {
    assert_eq!(tedious_rs::TDS_VERSION.len(), 6);
    assert_eq!(tedious_rs::TYPES.len(), 39);
    assert_eq!(tedious_rs::IsolationLevel::Snapshot as u8, 5);
    let bulk = tedious_rs::BulkLoad::new("dbo.items", tedious_rs::BulkLoadOptions::default());
    assert!(bulk.is_ok());
}

#[test]
fn exposes_node_facade_type_and_version_lookup_helpers() {
    assert_eq!(tedious_rs::type_by_name("Int").unwrap().id, 0x38);
    assert_eq!(tedious_rs::type_by_id(0x38).unwrap().name, "Int");
    assert_eq!(tedious_rs::versions()["7_4"], 0x74000004);
    assert_eq!(tedious_rs::versions_by_value()[&0x74000004], "7_4");
    let _: tedious_rs::ConnectionAuthentication = tedious_rs::Authentication::Sql {
        user_name: "user".to_owned(),
        password: "password".to_owned(),
    };
}

#[test]
fn exposes_statement_column_encryption_policy_from_the_facade() {
    assert_eq!(
        tedious_rs::StatementColumnEncryptionSetting::default(),
        tedious_rs::StatementColumnEncryptionSetting::UseConnectionSetting
    );
}

#[test]
fn exposes_connection_listener_event_names() {
    assert_eq!(ConnectionEventName::NewListener.as_str(), "newListener");
    assert_eq!(
        ConnectionEventName::RemoveListener.as_str(),
        "removeListener"
    );
    assert_eq!(
        String::from(ConnectionEventName::NewListener),
        "newListener"
    );
    assert_eq!(
        String::from(ConnectionEventName::RemoveListener),
        "removeListener"
    );
    assert!(CONNECTION_EVENT_NAMES.contains(&"newListener"));
    assert!(CONNECTION_EVENT_NAMES.contains(&"removeListener"));
}
