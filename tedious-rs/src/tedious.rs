pub use crate::bulk_load::BulkLoad;
pub use crate::connection::Connection;
pub use crate::data_type::TYPES;
pub use crate::errors::{ConnectionError, RequestError};
pub use crate::library::LIBRARY as library;
pub use crate::public_api::NodeConnectionFacade;
pub use crate::request::Request;
pub use crate::tds_versions::TDS_VERSION;
pub use crate::transaction::ISOLATION_LEVEL;

pub type ConnectionAuthentication = crate::connection::Authentication;
pub type ConnectionConfiguration = crate::connection::ConnectionConfig;
pub type ConnectionOptions = crate::connection::SessionOptions;

pub fn connect(config: ConnectionConfiguration) -> NodeConnectionFacade {
    NodeConnectionFacade::start(config, None::<fn(Option<ConnectionError>)>)
}

pub fn connect_with_callback<F>(
    config: ConnectionConfiguration,
    callback: F,
) -> NodeConnectionFacade
where
    F: FnOnce(Option<ConnectionError>) + Send + 'static,
{
    NodeConnectionFacade::start(config, Some(callback))
}
