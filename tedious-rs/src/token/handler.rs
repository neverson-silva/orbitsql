use std::fmt::{Display, Formatter};
use std::sync::Arc;

use super::parser::{
    AltMetadataToken, AltRowToken, ColInfoToken, DataClassificationToken, EnvChangeKind,
    EnvChangeToken, FeatureExtAckToken, FedAuthInfoToken, LoginAckToken, MessageToken, OffsetToken,
    ReturnStatusToken, ReturnValueToken, SessionStateToken, SspiToken, TabNameToken, Token,
};
use super::rows::{ColumnMetadata, Row};
use super::stream::StreamToken;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandlerEvent {
    Info(MessageToken),
    Error(MessageToken),
    DatabaseChange(String),
    LanguageChange(String),
    CharsetChange(String),
    PacketSize(u32),
    ResetConnection,
    RollbackTransaction,
    CollationChange(crate::collation::Collation),
    DatabaseMirroringPartner(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnexpectedTokenError {
    pub handler: &'static str,
    pub token: &'static str,
    pub category: &'static str,
    pub message: Option<String>,
    pub close_connection: bool,
}

impl Display for UnexpectedTokenError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(message) = &self.message {
            formatter.write_str(message)
        } else {
            write!(
                formatter,
                "Unexpected token `{}` in `{}`",
                self.token, self.handler
            )
        }
    }
}

impl std::error::Error for UnexpectedTokenError {}

pub trait TokenHandler {
    fn handler_name(&self) -> &'static str {
        std::any::type_name::<Self>()
            .rsplit("::")
            .next()
            .unwrap_or("TokenHandler")
    }

    fn on_info(&mut self, _token: &MessageToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("INFO"))
    }

    fn on_error(&mut self, _token: &MessageToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ERROR"))
    }

    fn on_sspi(&mut self, _token: &SspiToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("SSPI"))
    }

    fn on_env_change(&mut self, _token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ENVCHANGE"))
    }

    fn on_database_change(&mut self, _token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ENVCHANGE"))
    }

    fn on_language_change(&mut self, _token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ENVCHANGE"))
    }

    fn on_charset_change(&mut self, _token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ENVCHANGE"))
    }

    fn on_sql_collation_change(
        &mut self,
        _token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ENVCHANGE"))
    }

    fn on_packet_size_change(
        &mut self,
        _token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ENVCHANGE"))
    }

    fn on_reset_connection(&mut self, _token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ENVCHANGE"))
    }

    fn on_begin_transaction(
        &mut self,
        _token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ENVCHANGE"))
    }

    fn on_commit_transaction(
        &mut self,
        _token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ENVCHANGE"))
    }

    fn on_rollback_transaction(
        &mut self,
        _token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ENVCHANGE"))
    }

    fn on_database_mirroring_partner(
        &mut self,
        _token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ENVCHANGE"))
    }

    fn on_routing_change(&mut self, _token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ENVCHANGE"))
    }

    fn on_feature_ext_ack(
        &mut self,
        _token: &FeatureExtAckToken,
    ) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("FEATUREEXTACK"))
    }

    fn on_fed_auth_info(&mut self, _token: &FedAuthInfoToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("FEDAUTHINFO"))
    }

    fn on_login_ack(&mut self, _token: &LoginAckToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("LOGINACK"))
    }

    fn on_col_metadata(&mut self, _columns: &[ColumnMetadata]) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("COLMETADATA"))
    }

    fn on_order(&mut self, _order: &[u16]) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ORDER"))
    }

    fn on_row(&mut self, _row: &Row) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("ROW"))
    }

    fn on_nbc_row(&mut self, _row: &Row) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("NBCROW"))
    }

    fn on_return_status(&mut self, _token: &ReturnStatusToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("RETURNSTATUS"))
    }

    fn on_return_value(&mut self, _token: &ReturnValueToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("RETURNVALUE"))
    }

    fn on_done(&mut self, _token: &super::parser::DoneToken) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("DONE"))
    }

    fn on_done_proc(
        &mut self,
        _token: &super::parser::DoneToken,
    ) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("DONEPROC"))
    }

    fn on_done_in_proc(
        &mut self,
        _token: &super::parser::DoneToken,
    ) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected("DONEINPROC"))
    }

    fn on_offset(&mut self, _token: &OffsetToken) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }

    fn on_tab_name(&mut self, _token: &TabNameToken) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }

    fn on_col_info(&mut self, _token: &ColInfoToken) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }

    fn on_data_classification(
        &mut self,
        _token: &DataClassificationToken,
    ) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }

    fn on_alt_metadata(&mut self, _token: &AltMetadataToken) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }

    fn on_alt_row(&mut self, _token: &AltRowToken) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }

    fn on_session_state(&mut self, _token: &SessionStateToken) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }

    fn unexpected(&self, token: &'static str) -> UnexpectedTokenError {
        UnexpectedTokenError {
            handler: self.handler_name(),
            token,
            category: "unexpected_token",
            message: None,
            close_connection: false,
        }
    }
}

pub type InitialSqlCloseCallback = Arc<dyn Fn() + Send + Sync>;

pub fn dispatch_token<H: TokenHandler>(
    handler: &mut H,
    token: &Token,
) -> Result<(), UnexpectedTokenError> {
    match token {
        Token::Done(value) if value.token == 0xFD => handler.on_done(value),
        Token::Done(value) if value.token == 0xFE => handler.on_done_proc(value),
        Token::Done(value) => handler.on_done_in_proc(value),
        Token::Info(value) => handler.on_info(value),
        Token::Error(value) => handler.on_error(value),
        Token::ReturnStatus(value) => handler.on_return_status(value),
        Token::ReturnValue(value) => handler.on_return_value(value),
        Token::LoginAck(value) => handler.on_login_ack(value),
        Token::Sspi(value) => handler.on_sspi(value),
        Token::FeatureExtAck(value) => handler.on_feature_ext_ack(value),
        Token::FedAuthInfo(value) => handler.on_fed_auth_info(value),
        Token::Order(value) => handler.on_order(value),
        Token::EnvChange(value) => match value.change_type {
            1 => handler.on_database_change(value),
            2 => handler.on_language_change(value),
            3 => handler.on_charset_change(value),
            4 => handler.on_packet_size_change(value),
            7 => handler.on_sql_collation_change(value),
            8 => handler.on_begin_transaction(value),
            9 => handler.on_commit_transaction(value),
            10 => handler.on_rollback_transaction(value),
            13 => handler.on_database_mirroring_partner(value),
            18 => handler.on_reset_connection(value),
            20 => handler.on_routing_change(value),
            _ => handler.on_env_change(value),
        },
        Token::Offset(value) => handler.on_offset(value),
        Token::TabName(value) => handler.on_tab_name(value),
        Token::ColInfo(value) => handler.on_col_info(value),
        Token::DataClassification(value) => handler.on_data_classification(value),
        Token::AltMetadata(value) => handler.on_alt_metadata(value),
        Token::AltRow(value) => handler.on_alt_row(value),
        Token::SessionState(value) => handler.on_session_state(value),
    }
}

pub fn dispatch_stream_token<H: TokenHandler>(
    handler: &mut H,
    token: &StreamToken,
) -> Result<(), UnexpectedTokenError> {
    match token {
        StreamToken::Control(value) => dispatch_token(handler, value),
        StreamToken::ColMetadata(columns) => handler.on_col_metadata(columns),
        StreamToken::Row(row) => handler.on_row(row),
        StreamToken::NbcRow(row) => handler.on_nbc_row(row),
    }
}

#[derive(Clone, Default)]
pub struct InitialSqlTokenHandler {
    pub events: Vec<HandlerEvent>,
    pub transaction_descriptors: Vec<Vec<u8>>,
    pub in_transaction: bool,
    pub close_requested: bool,
    close_callback: Option<InitialSqlCloseCallback>,
}

impl InitialSqlTokenHandler {
    pub fn new() -> Self {
        Self {
            transaction_descriptors: vec![vec![0; 8]],
            ..Self::default()
        }
    }

    pub fn with_close_callback<F>(callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        Self {
            transaction_descriptors: vec![vec![0; 8]],
            close_callback: Some(Arc::new(callback)),
            ..Self::default()
        }
    }

    pub fn set_close_callback<F>(&mut self, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.close_callback = Some(Arc::new(callback));
    }
}

impl std::fmt::Debug for InitialSqlTokenHandler {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InitialSqlTokenHandler")
            .field("events", &self.events)
            .field("transaction_descriptors", &self.transaction_descriptors)
            .field("in_transaction", &self.in_transaction)
            .field("close_requested", &self.close_requested)
            .finish()
    }
}

impl PartialEq for InitialSqlTokenHandler {
    fn eq(&self, other: &Self) -> bool {
        self.events == other.events
            && self.transaction_descriptors == other.transaction_descriptors
            && self.in_transaction == other.in_transaction
            && self.close_requested == other.close_requested
    }
}

impl Eq for InitialSqlTokenHandler {}

impl TokenHandler for InitialSqlTokenHandler {
    fn on_info(&mut self, token: &MessageToken) -> Result<(), UnexpectedTokenError> {
        self.events.push(HandlerEvent::Info(token.clone()));
        Ok(())
    }

    fn on_error(&mut self, token: &MessageToken) -> Result<(), UnexpectedTokenError> {
        self.events.push(HandlerEvent::Error(token.clone()));
        Ok(())
    }

    fn on_env_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        apply_connection_environment(
            &mut self.events,
            &mut self.transaction_descriptors,
            &mut self.in_transaction,
            token,
        );
        Ok(())
    }

    fn on_database_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_language_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_charset_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_sql_collation_change(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_packet_size_change(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_reset_connection(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_begin_transaction(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_commit_transaction(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_rollback_transaction(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_col_metadata(&mut self, _columns: &[ColumnMetadata]) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected_initial_sql_token("COLMETADATA", "columnMetadata"))
    }

    fn on_order(&mut self, _order: &[u16]) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected_initial_sql_token("ORDER", "order"))
    }

    fn on_row(&mut self, _row: &Row) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected_initial_sql_token("ROW", "row"))
    }

    fn on_nbc_row(&mut self, _row: &Row) -> Result<(), UnexpectedTokenError> {
        Err(self.unexpected_initial_sql_token("NBCROW", "row"))
    }

    fn on_return_status(&mut self, _token: &ReturnStatusToken) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }

    fn on_return_value(&mut self, _token: &ReturnValueToken) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }

    fn on_done(&mut self, _token: &super::parser::DoneToken) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }

    fn on_done_proc(
        &mut self,
        _token: &super::parser::DoneToken,
    ) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }

    fn on_done_in_proc(
        &mut self,
        _token: &super::parser::DoneToken,
    ) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }
}

impl InitialSqlTokenHandler {
    fn unexpected_initial_sql_token(
        &mut self,
        token: &'static str,
        node_token_name: &'static str,
    ) -> UnexpectedTokenError {
        let first_close_request = !self.close_requested;
        self.close_requested = true;
        let error = UnexpectedTokenError {
            handler: self.handler_name(),
            token,
            category: "error",
            message: Some(format!(
                "Received '{node_token_name}' when no sqlRequest is in progress"
            )),
            close_connection: true,
        };
        if first_close_request && let Some(callback) = &self.close_callback {
            callback();
        }
        error
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingTarget {
    pub server: String,
    pub port: u16,
    pub instance: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Login7TokenHandler {
    pub events: Vec<HandlerEvent>,
    pub fed_auth_info: Option<FedAuthInfoToken>,
    pub routing_data: Option<RoutingTarget>,
    pub login_ack_received: bool,
    pub login_error: Option<crate::errors::ConnectionError>,
    pub sspi_packet: Option<SspiToken>,
    pub fed_auth_requested: bool,
    transient_retry_state: Option<(u32, u32)>,
}

impl Login7TokenHandler {
    pub fn new(fed_auth_requested: bool) -> Self {
        Self {
            fed_auth_requested,
            ..Self::default()
        }
    }

    pub fn with_transient_retry_state(
        fed_auth_requested: bool,
        transient_retry_count: u32,
        max_retries_on_transient_errors: u32,
    ) -> Self {
        Self {
            fed_auth_requested,
            transient_retry_state: Some((transient_retry_count, max_retries_on_transient_errors)),
            ..Self::default()
        }
    }
}

impl TokenHandler for Login7TokenHandler {
    fn on_info(&mut self, token: &MessageToken) -> Result<(), UnexpectedTokenError> {
        self.events.push(HandlerEvent::Info(token.clone()));
        Ok(())
    }

    fn on_error(&mut self, token: &MessageToken) -> Result<(), UnexpectedTokenError> {
        self.events.push(HandlerEvent::Error(token.clone()));
        let mut error =
            crate::errors::ConnectionError::new(token.message.clone(), Some("ELOGIN".to_owned()));
        if crate::transient_error_lookup::is_transient_error(token.number)
            && self
                .transient_retry_state
                .is_none_or(|(current, maximum)| current != maximum)
        {
            error.is_transient = Some(true);
        }
        self.login_error = Some(error);
        Ok(())
    }

    fn on_sspi(&mut self, token: &SspiToken) -> Result<(), UnexpectedTokenError> {
        self.sspi_packet = Some(token.clone());
        Ok(())
    }

    fn on_env_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        if let EnvChangeKind::Routing(route) = &token.kind {
            let (server, instance) = route
                .server
                .split_once('\\')
                .map_or((route.server.clone(), None), |(server, instance)| {
                    (server.to_owned(), Some(instance.to_owned()))
                });
            self.routing_data = Some(RoutingTarget {
                server,
                port: route.port,
                instance,
            });
        } else {
            let mut descriptors = Vec::new();
            let mut in_transaction = false;
            apply_connection_environment(
                &mut self.events,
                &mut descriptors,
                &mut in_transaction,
                token,
            );
        }
        Ok(())
    }

    fn on_database_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_language_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_charset_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_sql_collation_change(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_packet_size_change(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_reset_connection(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_begin_transaction(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_commit_transaction(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_rollback_transaction(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_database_mirroring_partner(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        if let EnvChangeKind::Text { new_value, .. } = &token.kind {
            self.events
                .push(HandlerEvent::DatabaseMirroringPartner(new_value.clone()));
        }
        Ok(())
    }

    fn on_routing_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_fed_auth_info(&mut self, token: &FedAuthInfoToken) -> Result<(), UnexpectedTokenError> {
        self.fed_auth_info = Some(token.clone());
        Ok(())
    }

    fn on_feature_ext_ack(
        &mut self,
        token: &FeatureExtAckToken,
    ) -> Result<(), UnexpectedTokenError> {
        if self.fed_auth_requested {
            if token.fed_auth.is_none() {
                self.login_error = Some(crate::errors::ConnectionError::new(
                    "Did not receive Active Directory authentication acknowledgement",
                    None,
                ));
            } else if token.fed_auth.as_ref().is_some_and(|data| !data.is_empty()) {
                self.login_error = Some(crate::errors::ConnectionError::new(
                    "Active Directory authentication acknowledgment includes extra data",
                    None,
                ));
            }
        } else if token.fed_auth.is_none() && token.utf8_support.is_none() {
            self.login_error = Some(crate::errors::ConnectionError::new(
                "Received acknowledgement for unknown feature",
                None,
            ));
        } else if token.fed_auth.is_some() {
            self.login_error = Some(crate::errors::ConnectionError::new(
                "Did not request Active Directory authentication, but received the acknowledgment",
                None,
            ));
        }
        Ok(())
    }

    fn on_login_ack(&mut self, token: &LoginAckToken) -> Result<(), UnexpectedTokenError> {
        if token.tds_version.is_none() {
            self.login_error = Some(crate::errors::ConnectionError::new(
                "Server responded with unknown TDS version.",
                Some("ETDS".to_owned()),
            ));
        } else if token.interface.is_none() {
            self.login_error = Some(crate::errors::ConnectionError::new(
                "Server responded with unsupported interface.",
                Some("EINTERFACENOTSUPP".to_owned()),
            ));
        } else {
            self.login_ack_received = true;
        }
        Ok(())
    }

    fn on_done(&mut self, _token: &super::parser::DoneToken) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }

    fn on_done_in_proc(
        &mut self,
        _token: &super::parser::DoneToken,
    ) -> Result<(), UnexpectedTokenError> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequestTokenHandler {
    pub events: Vec<crate::request::RequestEvent>,
    pub info_messages: Vec<MessageToken>,
    pub errors: Vec<crate::errors::RequestError>,
    pub error: Option<crate::errors::RequestError>,
    pub columns: Vec<ColumnMetadata>,
    pub rows: Vec<Row>,
    pub row_count: u64,
    pub return_status: Option<i32>,
    pub canceled: bool,
    pub row_collection_on_done: bool,
    pub row_collection_on_request_completion: bool,
    pub use_column_names: bool,
    pub environment_events: Vec<HandlerEvent>,
    pub transaction_descriptors: Vec<Vec<u8>>,
    pub in_transaction: bool,
    resultset_rows: Vec<Row>,
}

impl RequestTokenHandler {
    pub fn new(row_collection_on_done: bool, use_column_names: bool) -> Self {
        Self {
            events: Vec::new(),
            info_messages: Vec::new(),
            errors: Vec::new(),
            error: None,
            columns: Vec::new(),
            rows: Vec::new(),
            row_count: 0,
            return_status: None,
            canceled: false,
            row_collection_on_done,
            row_collection_on_request_completion: false,
            use_column_names,
            environment_events: Vec::new(),
            transaction_descriptors: vec![vec![0; 8]],
            in_transaction: false,
            resultset_rows: Vec::new(),
        }
    }

    pub fn set_row_collection_on_request_completion(&mut self, enabled: bool) {
        self.row_collection_on_request_completion = enabled;
    }

    fn emit(&mut self, event: crate::request::RequestEvent) {
        if !self.canceled {
            self.events.push(event);
        }
    }

    fn record_error(&mut self, error: crate::errors::RequestError) {
        self.errors.push(error.clone());
        self.error = if self.errors.len() == 1 {
            Some(error)
        } else {
            Some(crate::errors::RequestError::aggregate(self.errors.clone()))
        };
    }
}

impl TokenHandler for RequestTokenHandler {
    fn on_info(&mut self, token: &MessageToken) -> Result<(), UnexpectedTokenError> {
        if !self.canceled {
            self.info_messages.push(token.clone());
        }
        Ok(())
    }

    fn on_error(&mut self, token: &MessageToken) -> Result<(), UnexpectedTokenError> {
        if self.canceled {
            return Ok(());
        }
        let mut error =
            crate::errors::RequestError::new(token.message.clone(), Some("EREQUEST".to_owned()));
        error.number = i32::try_from(token.number).ok();
        error.state = Some(token.state);
        error.class = Some(token.class);
        error.server_name = Some(token.server_name.clone());
        error.proc_name = Some(token.proc_name.clone());
        error.line_number = i32::try_from(token.line_number).ok();
        error.serverName = Some(token.server_name.clone());
        error.procName = Some(token.proc_name.clone());
        error.lineNumber = i32::try_from(token.line_number).ok();
        self.record_error(error);
        Ok(())
    }

    fn on_col_metadata(&mut self, columns: &[ColumnMetadata]) -> Result<(), UnexpectedTokenError> {
        if !self.canceled {
            self.columns = if self.use_column_names {
                let mut unique = std::collections::HashSet::new();
                columns
                    .iter()
                    .filter(|column| unique.insert(column.col_name.clone()))
                    .cloned()
                    .collect()
            } else {
                columns.to_vec()
            };
            if self.use_column_names {
                let mut named_columns = crate::request::NamedColumnMetadata::new();
                for column in &self.columns {
                    named_columns.insert(column.col_name.clone(), column.clone());
                }
                self.emit(crate::request::RequestEvent::NamedColumnMetadata(
                    named_columns,
                ));
            } else {
                self.emit(crate::request::RequestEvent::ColumnMetadata(
                    self.columns.clone(),
                ));
            }
        }
        Ok(())
    }

    fn on_order(&mut self, order: &[u16]) -> Result<(), UnexpectedTokenError> {
        self.emit(crate::request::RequestEvent::Order(order.to_vec()));
        Ok(())
    }

    fn on_row(&mut self, row: &Row) -> Result<(), UnexpectedTokenError> {
        self.push_row(row.clone());
        Ok(())
    }

    fn on_nbc_row(&mut self, row: &Row) -> Result<(), UnexpectedTokenError> {
        self.push_row(row.clone());
        Ok(())
    }

    fn on_env_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        apply_connection_environment(
            &mut self.environment_events,
            &mut self.transaction_descriptors,
            &mut self.in_transaction,
            token,
        );
        Ok(())
    }

    fn on_database_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_language_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_charset_change(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_sql_collation_change(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_packet_size_change(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_reset_connection(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_begin_transaction(&mut self, token: &EnvChangeToken) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_commit_transaction(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_rollback_transaction(
        &mut self,
        token: &EnvChangeToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.on_env_change(token)
    }

    fn on_return_status(&mut self, token: &ReturnStatusToken) -> Result<(), UnexpectedTokenError> {
        if !self.canceled {
            self.return_status = Some(token.value);
        }
        Ok(())
    }

    fn on_return_value(&mut self, token: &ReturnValueToken) -> Result<(), UnexpectedTokenError> {
        self.emit(crate::request::RequestEvent::ReturnValue {
            name: token.name.clone(),
            value: token.value.clone(),
            metadata: Box::new(token.metadata.clone()),
        });
        Ok(())
    }

    fn on_done(&mut self, token: &super::parser::DoneToken) -> Result<(), UnexpectedTokenError> {
        self.finish_done(token, 0xFD);
        Ok(())
    }

    fn on_done_proc(
        &mut self,
        token: &super::parser::DoneToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.finish_done(token, 0xFE);
        Ok(())
    }

    fn on_done_in_proc(
        &mut self,
        token: &super::parser::DoneToken,
    ) -> Result<(), UnexpectedTokenError> {
        self.finish_done(token, 0xFF);
        Ok(())
    }
}

impl RequestTokenHandler {
    fn push_row(&mut self, row: Row) {
        if self.canceled {
            return;
        }
        if self.row_collection_on_request_completion {
            self.rows.push(row.clone());
        }
        if self.row_collection_on_done {
            self.resultset_rows.push(row.clone());
        }
        if self.use_column_names {
            self.emit(crate::request::RequestEvent::NamedRow(row.named_values()));
        } else {
            self.emit(crate::request::RequestEvent::Row(row));
        }
    }

    fn finish_done(&mut self, token: &super::parser::DoneToken, kind: u8) {
        if self.canceled {
            return;
        }
        if token.sql_error && self.error.is_none() {
            let error = crate::errors::RequestError::new(
                "An unknown error has occurred.",
                Some("UNKNOWN".to_owned()),
            );
            self.error = Some(error);
        }
        self.row_count = self.row_count.saturating_add(token.row_count.unwrap_or(0));
        let rows = self
            .row_collection_on_done
            .then(|| self.resultset_rows.clone());
        match kind {
            0xFE => self.emit(crate::request::RequestEvent::DoneProc {
                row_count: token.row_count,
                more: token.more,
                return_status: self.return_status,
                rows,
            }),
            0xFF => self.emit(crate::request::RequestEvent::DoneInProc {
                row_count: token.row_count,
                more: token.more,
                rows,
            }),
            _ => self.emit(crate::request::RequestEvent::Done {
                row_count: token.row_count,
                more: token.more,
                rows,
            }),
        }
        if kind == 0xFE {
            self.return_status = None;
        }
        if self.row_collection_on_done {
            self.resultset_rows.clear();
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttentionTokenHandler {
    pub attention_received: bool,
}

impl AttentionTokenHandler {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TokenHandler for AttentionTokenHandler {
    fn on_done(&mut self, token: &super::parser::DoneToken) -> Result<(), UnexpectedTokenError> {
        if token.attention {
            self.attention_received = true;
        }
        Ok(())
    }
}

fn apply_connection_environment(
    events: &mut Vec<HandlerEvent>,
    transaction_descriptors: &mut Vec<Vec<u8>>,
    in_transaction: &mut bool,
    token: &EnvChangeToken,
) {
    match (&token.kind, token.change_type) {
        (EnvChangeKind::Text { new_value, .. }, 1) => {
            events.push(HandlerEvent::DatabaseChange(new_value.clone()))
        }
        (EnvChangeKind::Text { new_value, .. }, 2) => {
            events.push(HandlerEvent::LanguageChange(new_value.clone()))
        }
        (EnvChangeKind::Text { new_value, .. }, 3) => {
            events.push(HandlerEvent::CharsetChange(new_value.clone()))
        }
        (EnvChangeKind::Text { new_value, .. }, 13) => {
            events.push(HandlerEvent::DatabaseMirroringPartner(new_value.clone()))
        }
        (EnvChangeKind::PacketSize { new_value, .. }, 4) => {
            events.push(HandlerEvent::PacketSize(*new_value))
        }
        (
            EnvChangeKind::Collation {
                new_value: Some(collation),
                ..
            },
            7,
        ) => events.push(HandlerEvent::CollationChange(collation.clone())),
        (EnvChangeKind::Binary { new_value, .. }, 7) => {
            if let Some(collation) = crate::collation::Collation::from_buffer(new_value, 0) {
                events.push(HandlerEvent::CollationChange(collation));
            }
        }
        (EnvChangeKind::BeginTransaction { new_value, .. }, 8)
        | (EnvChangeKind::Binary { new_value, .. }, 8) => {
            transaction_descriptors.push(new_value.clone());
            *in_transaction = true;
        }
        (EnvChangeKind::CommitTransaction { .. }, 9) | (EnvChangeKind::Binary { .. }, 9) => {
            transaction_descriptors.truncate(1);
            *in_transaction = false;
        }
        (EnvChangeKind::RollbackTransaction { .. }, 10) | (EnvChangeKind::Binary { .. }, 10) => {
            transaction_descriptors.truncate(1);
            *in_transaction = false;
            events.push(HandlerEvent::RollbackTransaction);
        }
        (EnvChangeKind::ResetConnection { .. }, 18) | (EnvChangeKind::Binary { .. }, 18) => {
            events.push(HandlerEvent::ResetConnection)
        }
        _ => {}
    }
}
