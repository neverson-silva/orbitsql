use orbitsql_ui_lib::backend::adapters::DatabaseDriverFactory;
#[path = "orbitsql_ui_connections.rs"]
mod orbitsql_ui_connections;
use orbitsql_ui_lib::backend::adapters::type_mapping::type_ref;
use orbitsql_ui_lib::backend::{
    CatalogProviderError, CatalogProviderFactory, CatalogRefreshRequest,
};
use orbitsql_ui_lib::catalog::{
    CatalogConstraint, CatalogSchema, CatalogSnapshot, CatalogTable, CatalogView, ColumnMetadata,
    DialectId, Identifier, IdentifierQuoteStyle, SqlTypeFamily, SqlTypeRef,
};

fn identifier(value: &str) -> Identifier {
    Identifier {
        raw: value.to_owned(),
        normalized: value.to_owned(),
        quoted: false,
        quote_style: IdentifierQuoteStyle::None,
    }
}

fn integer_type() -> SqlTypeRef {
    SqlTypeRef {
        family: SqlTypeFamily::Integer,
        name: "integer".to_owned(),
        nullable: Some(false),
        precision: None,
        scale: None,
        element_type: None,
    }
}

fn column() -> ColumnMetadata {
    ColumnMetadata {
        id: "column:users:1".to_owned(),
        table_id: "object:users".to_owned(),
        ordinal: 1,
        name: identifier("id"),
        r#type: integer_type(),
        nullable: false,
        generated: false,
        identity: true,
        computed: false,
        default_value: Some("nextval('users_id_seq')".to_owned()),
        primary_key: true,
        foreign_key_memberships: Vec::new(),
        unique_memberships: vec!["index:users:primary".to_owned()],
        comment: Some("identifier".to_owned()),
    }
}

fn snapshot() -> CatalogSnapshot {
    let schema = CatalogSchema {
        id: "schema:public".to_owned(),
        catalog_id: None,
        database_id: "database:test".to_owned(),
        name: identifier("public"),
        comment: Some("default schema".to_owned()),
    };
    let table = CatalogTable {
        id: "object:users".to_owned(),
        catalog_id: None,
        database_id: "database:test".to_owned(),
        schema_id: schema.id.clone(),
        name: identifier("users"),
        columns: vec![column()],
        primary_key: None,
        foreign_keys: Vec::new(),
        unique_constraints: Vec::new(),
        check_constraints: Vec::new(),
        indexes: Vec::new(),
        estimated_rows: Some(3),
        comment: Some("accounts".to_owned()),
        temporary: false,
        foreign: false,
    };
    let view = CatalogView {
        id: "object:user_names".to_owned(),
        catalog_id: None,
        database_id: "database:test".to_owned(),
        schema_id: schema.id.clone(),
        name: identifier("user_names"),
        columns: vec![column()],
        comment: Some("projection".to_owned()),
    };
    CatalogSnapshot {
        schema_version: 1,
        id: "catalog:connection:1".to_owned(),
        connection_id: "connection:test".to_owned(),
        database_id: "database:test".to_owned(),
        dialect: DialectId::Postgresql,
        version: "catalog-v1".to_owned(),
        generated_at: "2026-01-01T00:00:00Z".to_owned(),
        databases: Vec::new(),
        schemas: vec![schema],
        tables: vec![table],
        views: vec![view],
        materialized_views: Vec::new(),
        functions: Vec::new(),
        procedures: Vec::new(),
        sequences: Vec::new(),
        types: Vec::new(),
        triggers: Vec::new(),
        extensions: Vec::new(),
        partitions: Vec::new(),
        principals: Vec::new(),
        permissions: Vec::new(),
        synonyms: Vec::new(),
        foreign_keys: Vec::new(),
        indexes: Vec::new(),
        constraints: Vec::<CatalogConstraint>::new(),
    }
}

#[test]
fn catalog_snapshot_serializes_and_round_trips_the_canonical_contract() {
    let value = serde_json::to_value(snapshot()).expect("snapshot must serialize");
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["dialect"], "postgresql");
    assert_eq!(value["schemas"][0]["databaseId"], "database:test");
    assert_eq!(value["tables"][0]["columns"][0]["tableId"], "object:users");
    assert_eq!(
        value["tables"][0]["columns"][0]["defaultValue"],
        "nextval('users_id_seq')"
    );
    let decoded: CatalogSnapshot = serde_json::from_value(value).expect("snapshot must decode");
    assert_eq!(decoded, snapshot());
}

#[test]
fn type_normalization_preserves_families_precision_arrays_and_vendor_timestamp_rules() {
    assert_eq!(
        type_ref("numeric(12,2)", 12, 2).family,
        SqlTypeFamily::Decimal
    );
    assert_eq!(type_ref("jsonb", 0, 0).family, SqlTypeFamily::Json);
    assert_eq!(type_ref("uuid", 0, 0).family, SqlTypeFamily::Uuid);
    assert_eq!(type_ref("integer[]", 0, 0).family, SqlTypeFamily::Array);
    assert_eq!(type_ref("numeric(12,2)", 12, 2).precision, Some(12));
    assert_eq!(type_ref("numeric(12,2)", 12, 2).scale, Some(2));
}

#[test]
fn database_driver_factory_exposes_only_the_enabled_catalog_drivers() {
    let factory = DatabaseDriverFactory::default();
    assert_eq!(
        factory
            .provider(DialectId::Postgresql)
            .expect("PostgreSQL adapter must be registered")
            .dialect(),
        DialectId::Postgresql
    );
    assert!(matches!(
        factory.provider(DialectId::Sqlserver),
        Ok(provider) if provider.dialect() == DialectId::Sqlserver
    ));
    assert!(matches!(
        factory.provider(DialectId::Mysql),
        Err(CatalogProviderError::UnsupportedDialect(DialectId::Mysql))
    ));
    assert!(matches!(
        factory.provider(DialectId::Mariadb),
        Err(CatalogProviderError::UnsupportedDialect(DialectId::Mariadb))
    ));
}

#[test]
fn provider_errors_preserve_unsupported_and_driver_diagnostics() {
    let unsupported = CatalogProviderError::UnsupportedDialect(DialectId::Oracle);
    assert_eq!(
        unsupported.to_string(),
        "No catalog provider is registered for Oracle"
    );
    let driver = CatalogProviderError::Driver {
        dialect: DialectId::Postgresql,
        message: "connection failed".to_owned(),
    };
    assert_eq!(
        driver.to_string(),
        "Postgresql catalog provider failed: connection failed"
    );
}

#[test]
fn catalog_refresh_request_debug_redacts_password() {
    let request = CatalogRefreshRequest {
        dialect: DialectId::Sqlserver,
        connection_id: "connection:test".to_owned(),
        version: "catalog-v1".to_owned(),
        generated_at: "2026-01-01T00:00:00Z".to_owned(),
        server: "localhost".to_owned(),
        port: 1433,
        user_name: "sa".to_owned(),
        password: "not-safe-to-print".to_owned(),
        database: "master".to_owned(),
        trust_server_certificate: true,
        auth_method: None,
        show_all_catalog_items: false,
        transport: Default::default(),
        driver_properties: Default::default(),
    };
    let debug = format!("{request:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("not-safe-to-print"));
}

#[tokio::test]
async fn driver_factory_resolves_sqlserver_without_starting_network_io() {
    let provider = DatabaseDriverFactory::default()
        .provider(DialectId::Sqlserver)
        .expect("SQL Server adapter must be registered");
    assert_eq!(provider.dialect(), DialectId::Sqlserver);
}
