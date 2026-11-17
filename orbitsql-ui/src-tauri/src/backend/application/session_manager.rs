use crate::backend::domain::connection::ConnectionOperationError;
use crate::backend::domain::session_manager::{
    SessionKillResult, SessionTelemetry, validate_kill_session_id,
};
use crate::backend::ports::connection::ConnectionRuntimeRegistry;

fn connection_error() -> ConnectionOperationError {
    ConnectionOperationError::new(
        "CONNECTION_NOT_CONNECTED",
        "Connection is not connected",
        true,
    )
}

pub async fn get_database_sessions(
    registry: &ConnectionRuntimeRegistry,
    connection_id: String,
) -> Result<SessionTelemetry, ConnectionOperationError> {
    let connection = registry.get(&connection_id).ok_or_else(connection_error)?;
    registry.touch(&connection_id);
    connection.session_telemetry().await
}

pub async fn kill_database_session(
    registry: &ConnectionRuntimeRegistry,
    connection_id: String,
    session_id: i32,
) -> Result<SessionKillResult, ConnectionOperationError> {
    let connection = registry.get(&connection_id).ok_or_else(connection_error)?;
    registry.touch(&connection_id);
    let telemetry = connection.session_telemetry().await?;
    validate_kill_session_id(session_id, telemetry.monitor_session_id)?;
    connection.kill_session(session_id).await
}
