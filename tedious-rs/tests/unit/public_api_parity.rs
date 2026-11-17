use tedious_rs::tedious::{
    BulkLoad as TediousBulkLoad, Connection as TediousConnection,
    ConnectionAuthentication as TediousConnectionAuthentication,
    ConnectionConfiguration as TediousConnectionConfiguration,
    ConnectionOptions as TediousConnectionOptions, Request as TediousRequest,
};
use tedious_rs::{BulkLoadEvent, ConnectionConfig, ConnectionEvent, RequestEvent, RequestResult};

#[test]
fn exposes_tedious_facade_symbols_and_node_constructors() {
    let config = ConnectionConfig::sql("localhost", "user", "password");
    let _: TediousConnectionOptions = config.session.clone();
    let _: TediousConnection = TediousConnection::new(config.clone());
    let _: Result<TediousBulkLoad, String> = TediousBulkLoad::new("dbo.items", Default::default());
    let _: TediousConnectionAuthentication = config.authentication;
    let _: TediousConnectionConfiguration = config;
    let _: TediousRequest = TediousRequest::new("select 1", |_| {});
    assert_eq!(tedious_rs::tedious::library.name, "Tedious");
    assert_eq!(tedious_rs::tedious::TYPES.len(), 39);
    assert_eq!(tedious_rs::tedious::ISOLATION_LEVEL.len(), 6);
    assert_eq!(tedious_rs::tedious::TDS_VERSION.len(), 6);
}

#[test]
fn exposes_public_event_types_from_the_package_root() {
    let _: Option<ConnectionEvent> = None;
    let _: Option<RequestEvent> = None;
    let _: Option<BulkLoadEvent> = None;
    let _: Option<RequestResult> = None;
}
