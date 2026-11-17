use crate::always_encrypted::types::StatementColumnEncryptionSetting;
use crate::data_type::type_for_parameter_type;
use crate::errors::RequestError;
use crate::metadata_parser::Metadata;
use crate::parameter::{ParameterOptions, SqlParameterType};
use crate::payloads::RpcParameter;
use crate::token::rows::{ColumnMetadata, Row};
use crate::value_codec::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Notify;

pub type RequestListenerId = u64;
type RequestListener = Arc<dyn Fn(&RequestEvent) + Send + Sync>;
type RequestMetaListener = Arc<dyn Fn(&RequestMetaEvent) + Send + Sync>;
struct RequestListenerEntry {
    id: RequestListenerId,
    name: Option<String>,
    once: bool,
    callback: RequestListener,
}
struct RequestMetaListenerEntry {
    id: RequestListenerId,
    event: RequestMetaEventKind,
    once: bool,
    callback: RequestMetaListener,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestMetaEventKind {
    NewListener,
    RemoveListener,
}
type CompletionCallback = Box<dyn FnMut(Result<RequestResult, RequestError>) + Send>;

#[derive(Clone)]
pub struct RequestCancellationHandle {
    cancelled: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NamedColumnMetadata {
    entries: Vec<(String, ColumnMetadata)>,
}

impl NamedColumnMetadata {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn insert(&mut self, name: String, column: ColumnMetadata) {
        if !self.contains_key(&name) {
            self.entries.push((name, column));
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains_key(&self, name: &str) -> bool {
        self.entries.iter().any(|(key, _)| key == name)
    }

    pub fn get(&self, name: &str) -> Option<&ColumnMetadata> {
        self.entries
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, column)| column)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &ColumnMetadata)> {
        self.entries.iter().map(|(name, column)| (name, column))
    }
}

impl Default for NamedColumnMetadata {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestCancellationHandle {
    pub fn cancel(&self) {
        if !self.cancelled.swap(true, Ordering::AcqRel) {
            self.notify.notify_waiters();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub async fn cancelled(&self) {
        loop {
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RequestOptions {
    pub statement_column_encryption_setting: StatementColumnEncryptionSetting,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RequestEvent {
    ColumnMetadata(Vec<ColumnMetadata>),
    NamedColumnMetadata(NamedColumnMetadata),
    Row(Row),
    NamedRow(std::collections::BTreeMap<String, Vec<Value>>),
    Done {
        row_count: Option<u64>,
        more: bool,
        rows: Option<Vec<Row>>,
    },
    DoneProc {
        row_count: Option<u64>,
        more: bool,
        return_status: Option<i32>,
        rows: Option<Vec<Row>>,
    },
    DoneInProc {
        row_count: Option<u64>,
        more: bool,
        rows: Option<Vec<Row>>,
    },
    Error(RequestError),
    RequestCompleted,
    Cancel,
    Pause,
    Resume,
    ReturnValue {
        name: String,
        value: crate::value_codec::Value,
        metadata: Box<Metadata>,
    },
    Order(Vec<u16>),
    Prepared,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestMetaEvent {
    NewListener {
        event: String,
        listener_id: RequestListenerId,
    },
    RemoveListener {
        event: String,
        listener_id: RequestListenerId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestEmitError {
    UnhandledError,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequestResult {
    pub row_count: usize,
    pub rows: Vec<Row>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestState {
    Created,
    Running,
    Canceled,
    Completed,
    Failed,
}

pub struct Request {
    pub sql_text: String,
    pub state: RequestState,
    pub rows: Vec<Row>,
    pub row_count: u64,
    pub errors: Vec<RequestError>,
    pub return_status: Option<i32>,
    pub error: Option<RequestError>,
    pub parameters: Vec<RpcParameter>,
    pub crypto_metadata_loaded: bool,
    pub crypto_metadata: Option<Vec<crate::always_encrypted::types::CryptoMetadata>>,
    force_encrypt_parameters: Vec<bool>,
    pub canceled: bool,
    pub paused: bool,
    pub timeout: Option<Duration>,
    pub row_collection_on_completion: bool,
    pub statement_column_encryption_setting: StatementColumnEncryptionSetting,
    pub handle: Option<i32>,
    pub preparing: bool,
    pub typed_parameters: Vec<(String, SqlParameterType, ParameterOptions)>,
    typed_parameter_values: Vec<(String, crate::data_type::DataType, ParameterOptions, Value)>,
    typed_parameter_indices: Vec<usize>,
    pending_events: Vec<RequestEvent>,
    listeners: Vec<RequestListenerEntry>,
    meta_listeners: Vec<RequestMetaListenerEntry>,
    next_listener_id: RequestListenerId,
    callback: Option<CompletionCallback>,
    completion_notified: bool,
    cancellation: RequestCancellationHandle,
}

impl Request {
    pub fn new(
        sql_text: impl Into<String>,
        callback: impl FnMut(Result<RequestResult, RequestError>) + Send + 'static,
    ) -> Self {
        Self::with_options(sql_text, callback, RequestOptions::default())
    }

    pub fn with_options(
        sql_text: impl Into<String>,
        callback: impl FnMut(Result<RequestResult, RequestError>) + Send + 'static,
        options: RequestOptions,
    ) -> Self {
        Self {
            sql_text: sql_text.into(),
            state: RequestState::Created,
            rows: Vec::new(),
            row_count: 0,
            errors: Vec::new(),
            return_status: None,
            error: None,
            parameters: Vec::new(),
            crypto_metadata_loaded: false,
            crypto_metadata: None,
            force_encrypt_parameters: Vec::new(),
            canceled: false,
            paused: false,
            timeout: None,
            row_collection_on_completion: false,
            statement_column_encryption_setting: options.statement_column_encryption_setting,
            handle: None,
            preparing: false,
            typed_parameters: Vec::new(),
            typed_parameter_values: Vec::new(),
            typed_parameter_indices: Vec::new(),
            pending_events: Vec::new(),
            listeners: Vec::new(),
            meta_listeners: Vec::new(),
            next_listener_id: 0,
            callback: Some(Box::new(callback)),
            completion_notified: false,
            cancellation: RequestCancellationHandle {
                cancelled: Arc::new(AtomicBool::new(false)),
                notify: Arc::new(Notify::new()),
            },
        }
    }

    pub fn new_optional<F>(sql_text: Option<impl Into<String>>, callback: F) -> Self
    where
        F: FnMut(Option<RequestError>, usize, Vec<Row>) + Send + 'static,
    {
        Self::with_node_callback(sql_text, callback, RequestOptions::default())
    }

    pub fn with_node_callback<F>(
        sql_text: Option<impl Into<String>>,
        mut callback: F,
        options: RequestOptions,
    ) -> Self
    where
        F: FnMut(Option<RequestError>, usize, Vec<Row>) + Send + 'static,
    {
        Self::with_options(
            sql_text.map(Into::into).unwrap_or_default(),
            move |result| match result {
                Ok(result) => callback(None, result.row_count, result.rows),
                Err(error) => callback(Some(error), 0, Vec::new()),
            },
            options,
        )
    }

    pub fn sql_text_or_procedure(&self) -> Option<&str> {
        if self.sql_text.is_empty() {
            None
        } else {
            Some(&self.sql_text)
        }
    }

    pub fn cancellation_handle(&self) -> RequestCancellationHandle {
        self.cancellation.clone()
    }

    pub fn add_parameter(
        &mut self,
        name: impl Into<String>,
        data_type: crate::data_type::DataType,
        value: Value,
        options: ParameterOptions,
    ) -> Result<(), crate::value_codec::CodecError> {
        let output = options.output;
        self.add_parameter_value_with_output(name, data_type, value, options, output, false)
    }

    pub fn add_parameter_with_force_encrypt(
        &mut self,
        name: impl Into<String>,
        data_type: crate::data_type::DataType,
        value: Value,
        options: ParameterOptions,
        force_encrypt: bool,
    ) -> Result<(), crate::value_codec::CodecError> {
        let name = name.into();
        self.add_parameter(name.clone(), data_type, value, options)?;
        self.set_parameter_force_encrypt(&name, force_encrypt)
            .map_err(|error| crate::value_codec::CodecError(error.message))
    }

    pub fn add_parameter_optional(
        &mut self,
        name: impl Into<String>,
        data_type: crate::data_type::DataType,
        value: Option<Value>,
        options: Option<ParameterOptions>,
    ) -> Result<(), crate::value_codec::CodecError> {
        let output = options.as_ref().is_some_and(|value| value.output);
        let options = options.unwrap_or_default();
        self.add_parameter_value_with_output(
            name,
            data_type,
            value.unwrap_or(Value::Null),
            ParameterOptions { output, ..options },
            output,
            true,
        )
    }

    pub fn add_raw_parameter(
        &mut self,
        name: impl Into<String>,
        encoded: crate::value_codec::EncodedParameter,
    ) {
        self.parameters.push(RpcParameter {
            name: name.into(),
            output: false,
            encoded,
            encrypted_metadata: None,
        });
        self.force_encrypt_parameters.push(false);
    }

    pub fn add_raw_parameter_with_force_encrypt(
        &mut self,
        name: impl Into<String>,
        encoded: crate::value_codec::EncodedParameter,
        force_encrypt: bool,
    ) {
        let name = name.into();
        self.add_raw_parameter(name.clone(), encoded);
        if let Some(index) = self.parameters.len().checked_sub(1) {
            self.force_encrypt_parameters[index] = force_encrypt;
        }
    }

    pub fn add_output_parameter(
        &mut self,
        name: impl Into<String>,
        data_type: crate::data_type::DataType,
        value: Value,
        options: ParameterOptions,
    ) -> Result<(), crate::value_codec::CodecError> {
        self.add_output_parameter_value(name, data_type, value, options)
    }

    pub fn add_output_parameter_with_force_encrypt(
        &mut self,
        name: impl Into<String>,
        data_type: crate::data_type::DataType,
        value: Value,
        options: ParameterOptions,
        force_encrypt: bool,
    ) -> Result<(), crate::value_codec::CodecError> {
        let name = name.into();
        self.add_output_parameter(name.clone(), data_type, value, options)?;
        self.set_parameter_force_encrypt(&name, force_encrypt)
            .map_err(|error| crate::value_codec::CodecError(error.message))
    }

    pub fn add_output_parameter_optional(
        &mut self,
        name: impl Into<String>,
        data_type: crate::data_type::DataType,
        value: Option<Value>,
        options: Option<ParameterOptions>,
    ) -> Result<(), crate::value_codec::CodecError> {
        self.add_output_parameter_value(
            name,
            data_type,
            value.unwrap_or(Value::Null),
            options.unwrap_or_default(),
        )
    }

    pub fn add_raw_output_parameter(
        &mut self,
        name: impl Into<String>,
        encoded: crate::value_codec::EncodedParameter,
    ) {
        self.parameters.push(RpcParameter {
            name: name.into(),
            output: true,
            encoded,
            encrypted_metadata: None,
        });
        self.force_encrypt_parameters.push(false);
    }

    pub fn add_raw_output_parameter_with_force_encrypt(
        &mut self,
        name: impl Into<String>,
        encoded: crate::value_codec::EncodedParameter,
        force_encrypt: bool,
    ) {
        let name = name.into();
        self.add_raw_output_parameter(name, encoded);
        if let Some(index) = self.parameters.len().checked_sub(1) {
            self.force_encrypt_parameters[index] = force_encrypt;
        }
    }

    pub fn add_parameter_value(
        &mut self,
        name: impl Into<String>,
        data_type: crate::data_type::DataType,
        value: Value,
        options: ParameterOptions,
    ) -> Result<(), crate::value_codec::CodecError> {
        self.add_parameter_value_with_output(name, data_type, value, options, false, true)
    }

    pub fn add_output_parameter_value(
        &mut self,
        name: impl Into<String>,
        data_type: crate::data_type::DataType,
        value: Value,
        options: ParameterOptions,
    ) -> Result<(), crate::value_codec::CodecError> {
        self.add_parameter_value_with_output(name, data_type, value, options, true, true)
    }

    fn add_parameter_value_with_output(
        &mut self,
        name: impl Into<String>,
        data_type: crate::data_type::DataType,
        value: Value,
        options: ParameterOptions,
        output: bool,
        encode_now: bool,
    ) -> Result<(), crate::value_codec::CodecError> {
        let parameter_type = data_type.parameter_type.ok_or_else(|| {
            crate::value_codec::CodecError(format!(
                "{} cannot be used as a parameter",
                data_type.name
            ))
        })?;
        let name = name.into();
        let encoded = if !encode_now
            || matches!(
                parameter_type,
                SqlParameterType::VarChar | SqlParameterType::Char | SqlParameterType::Text
            ) {
            crate::value_codec::EncodedParameter {
                type_info: Vec::new(),
                length: Vec::new(),
                data: Vec::new(),
            }
        } else {
            data_type.encode(&value, &options)?
        };
        self.parameters.push(RpcParameter {
            name: name.clone(),
            output,
            encoded,
            encrypted_metadata: None,
        });
        self.typed_parameter_indices.push(self.parameters.len() - 1);
        self.force_encrypt_parameters.push(false);
        self.typed_parameters
            .push((name.clone(), parameter_type, options.clone()));
        self.typed_parameter_values
            .push((name, data_type, options, value));
        Ok(())
    }

    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }

    pub fn set_row_collection_on_completion(&mut self, enabled: bool) {
        self.row_collection_on_completion = enabled;
    }

    pub fn should_honor_column_encryption(&self, connection_setting: bool) -> bool {
        crate::always_encrypted::utils::should_honor_ae(
            self.statement_column_encryption_setting,
            connection_setting,
        )
    }

    pub fn force_encrypt_parameters(&self) -> &[bool] {
        &self.force_encrypt_parameters
    }

    #[allow(clippy::result_large_err)]
    pub fn set_parameter_force_encrypt(
        &mut self,
        name: &str,
        force_encrypt: bool,
    ) -> Result<(), RequestError> {
        let index = self
            .parameters
            .iter()
            .rposition(|parameter| parameter.name == name)
            .ok_or_else(|| {
                RequestError::new(
                    format!("unknown parameter {name}"),
                    Some("EPARAM".to_owned()),
                )
            })?;
        self.force_encrypt_parameters[index] = force_encrypt;
        Ok(())
    }

    pub fn add_typed_parameter(
        &mut self,
        name: impl Into<String>,
        parameter_type: SqlParameterType,
        value: Value,
        options: ParameterOptions,
    ) -> Result<(), crate::value_codec::CodecError> {
        let name = name.into();
        let encoded = encode_typed_parameter(parameter_type, &value, &options)?;
        self.parameters.push(RpcParameter {
            name: name.clone(),
            output: false,
            encoded,
            encrypted_metadata: None,
        });
        self.typed_parameter_indices.push(self.parameters.len() - 1);
        self.force_encrypt_parameters.push(false);
        self.typed_parameters
            .push((name.clone(), parameter_type, options.clone()));
        let data_type = type_for_parameter_type(parameter_type).ok_or_else(|| {
            crate::value_codec::CodecError(format!("unknown parameter type for {name}"))
        })?;
        self.typed_parameter_values.push((
            self.parameters
                .last()
                .map(|parameter| parameter.name.clone())
                .unwrap_or_default(),
            data_type,
            options,
            value,
        ));
        Ok(())
    }

    pub fn add_typed_output_parameter(
        &mut self,
        name: impl Into<String>,
        parameter_type: SqlParameterType,
        value: Value,
        options: ParameterOptions,
    ) -> Result<(), crate::value_codec::CodecError> {
        let name = name.into();
        let encoded = encode_typed_parameter(parameter_type, &value, &options)?;
        self.parameters.push(RpcParameter {
            name: name.clone(),
            output: true,
            encoded,
            encrypted_metadata: None,
        });
        self.typed_parameter_indices.push(self.parameters.len() - 1);
        self.force_encrypt_parameters.push(false);
        self.typed_parameters
            .push((name.clone(), parameter_type, options.clone()));
        let data_type = type_for_parameter_type(parameter_type).ok_or_else(|| {
            crate::value_codec::CodecError(format!("unknown parameter type for {name}"))
        })?;
        self.typed_parameter_values.push((
            self.parameters
                .last()
                .map(|parameter| parameter.name.clone())
                .unwrap_or_default(),
            data_type,
            options,
            value,
        ));
        Ok(())
    }

    pub fn set_typed_parameter_value(
        &mut self,
        name: &str,
        value: Value,
    ) -> Result<(), crate::value_codec::CodecError> {
        let matching_indices = self
            .typed_parameters
            .iter()
            .enumerate()
            .filter_map(|(index, (parameter_name, _, _))| (parameter_name == name).then_some(index))
            .collect::<Vec<_>>();
        if matching_indices.is_empty() {
            return Err(crate::value_codec::CodecError(format!(
                "unknown parameter {name}"
            )));
        }
        for typed_index in matching_indices {
            let parameter_index = self.typed_parameter_indices[typed_index];
            let (_, parameter_type, options) = self.typed_parameters[typed_index].clone();
            let encoded = encode_typed_parameter(parameter_type, &value, &options)?;
            let parameter = self.parameters.get_mut(parameter_index).ok_or_else(|| {
                crate::value_codec::CodecError(format!("unknown parameter {name}"))
            })?;
            parameter.encoded = encoded;
            self.typed_parameter_values[typed_index].3 = value.clone();
        }
        Ok(())
    }

    pub fn reencode_typed_parameters(
        &mut self,
        collation: Option<&crate::collation::Collation>,
    ) -> Result<(), crate::value_codec::CodecError> {
        let typed_parameter_values = self.typed_parameter_values.clone();
        for (typed_index, (name, data_type, options, value)) in
            typed_parameter_values.into_iter().enumerate()
        {
            let mut effective_options = options.clone();
            if collation.is_some() {
                effective_options.collation = collation.cloned();
            }
            let encoded = data_type.encode(&value, &effective_options)?;
            let parameter_index = self.typed_parameter_indices[typed_index];
            let parameter = self.parameters.get_mut(parameter_index).ok_or_else(|| {
                crate::value_codec::CodecError(format!("unknown parameter {name}"))
            })?;
            parameter.encoded = encoded;
        }
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    pub fn validate_parameters(
        &mut self,
        collation: Option<&crate::collation::Collation>,
    ) -> Result<(), RequestError> {
        let values = self.typed_parameter_values.clone();
        for (typed_index, (name, data_type, options, value)) in values.into_iter().enumerate() {
            let parameter_type = data_type.parameter_type.ok_or_else(|| {
                RequestError::new(
                    format!("Validation failed for parameter '{name}'."),
                    Some("EPARAM".to_owned()),
                )
            })?;
            if matches!(
                parameter_type,
                SqlParameterType::VarChar | SqlParameterType::Char | SqlParameterType::Text
            ) && !matches!(value, Value::Null)
            {
                let collation = collation.ok_or_else(|| {
                    RequestError::new(
                        format!(
                            "Validation failed for parameter '{name}'. No collation was set by the server for the current connection."
                        ),
                        Some("EPARAM".to_owned()),
                    )
                })?;
                if collation.codepage.is_none() {
                    return Err(RequestError::new(
                        format!(
                            "Validation failed for parameter '{name}'. The collation set by the server has no associated encoding."
                        ),
                        Some("EPARAM".to_owned()),
                    ));
                }
            }
            let mut effective_options = options;
            if let Some(collation) = collation {
                effective_options.collation = Some(collation.clone());
            }
            let encoded = data_type
                .encode(&value, &effective_options)
                .map_err(|error| {
                    RequestError::with_cause(
                        format!("Validation failed for parameter '{name}'. {error}"),
                        Some("EPARAM".to_owned()),
                        error,
                    )
                })?;
            let parameter_index = self.typed_parameter_indices[typed_index];
            let parameter = self.parameters.get_mut(parameter_index).ok_or_else(|| {
                RequestError::new(
                    format!("Validation failed for parameter '{name}'. parameter is missing"),
                    Some("EPARAM".to_owned()),
                )
            })?;
            parameter.encoded = encoded;
        }
        Ok(())
    }

    pub fn make_params_declaration(&self) -> Result<String, crate::value_codec::CodecError> {
        let mut declarations = Vec::with_capacity(self.parameters.len());
        for (parameter_index, parameter) in self.parameters.iter().enumerate() {
            if let Some(typed_index) = self
                .typed_parameter_indices
                .iter()
                .position(|index| *index == parameter_index)
            {
                let (_, data_type, options, value) = &self.typed_parameter_values[typed_index];
                let declaration_options = if options.length.is_none()
                    && matches!(
                        data_type.parameter_type,
                        Some(SqlParameterType::VarChar) | Some(SqlParameterType::Char)
                    )
                    && parameter.encoded.type_info.len() >= 3
                {
                    ParameterOptions {
                        length: Some(usize::from(u16::from_le_bytes([
                            parameter.encoded.type_info[1],
                            parameter.encoded.type_info[2],
                        ]))),
                        ..options.clone()
                    }
                } else {
                    options.clone()
                };
                let declaration =
                    data_type.declaration(value, &declaration_options, parameter.output)?;
                declarations.push(format!(
                    "@{} {}{}",
                    parameter.name,
                    declaration,
                    if parameter.output { " OUTPUT" } else { "" }
                ));
            } else {
                let output = if parameter.output { " OUTPUT" } else { "" };
                declarations.push(format!(
                    "@{} {}{}",
                    parameter.name,
                    crate::connection::parameter_sql_declaration_for_public(&parameter.encoded),
                    output
                ));
            }
        }
        Ok(declarations.join(", "))
    }

    pub fn make_params_parameter(&self) -> Result<String, crate::value_codec::CodecError> {
        self.make_params_declaration()
    }

    pub fn on(
        &mut self,
        listener: impl Fn(&RequestEvent) + Send + Sync + 'static,
    ) -> RequestListenerId {
        self.add_listener(None, listener, false)
    }

    pub fn once(
        &mut self,
        listener: impl Fn(&RequestEvent) + Send + Sync + 'static,
    ) -> RequestListenerId {
        self.add_listener(None, listener, true)
    }

    pub fn on_named(
        &mut self,
        name: impl Into<String>,
        listener: impl Fn(&RequestEvent) + Send + Sync + 'static,
    ) -> RequestListenerId {
        self.add_listener(Some(name.into()), listener, false)
    }

    pub fn once_named(
        &mut self,
        name: impl Into<String>,
        listener: impl Fn(&RequestEvent) + Send + Sync + 'static,
    ) -> RequestListenerId {
        self.add_listener(Some(name.into()), listener, true)
    }

    pub fn on_new_listener(
        &mut self,
        listener: impl Fn(&RequestMetaEvent) + Send + Sync + 'static,
    ) -> RequestListenerId {
        self.add_meta_listener(RequestMetaEventKind::NewListener, listener, false)
    }

    pub fn once_new_listener(
        &mut self,
        listener: impl Fn(&RequestMetaEvent) + Send + Sync + 'static,
    ) -> RequestListenerId {
        self.add_meta_listener(RequestMetaEventKind::NewListener, listener, true)
    }

    pub fn on_remove_listener(
        &mut self,
        listener: impl Fn(&RequestMetaEvent) + Send + Sync + 'static,
    ) -> RequestListenerId {
        self.add_meta_listener(RequestMetaEventKind::RemoveListener, listener, false)
    }

    pub fn once_remove_listener(
        &mut self,
        listener: impl Fn(&RequestMetaEvent) + Send + Sync + 'static,
    ) -> RequestListenerId {
        self.add_meta_listener(RequestMetaEventKind::RemoveListener, listener, true)
    }

    fn add_meta_listener(
        &mut self,
        event: RequestMetaEventKind,
        listener: impl Fn(&RequestMetaEvent) + Send + Sync + 'static,
        once: bool,
    ) -> RequestListenerId {
        let id = self.next_listener_id;
        self.next_listener_id = self.next_listener_id.wrapping_add(1);
        self.emit_meta(RequestMetaEvent::NewListener {
            event: meta_event_name(event).to_owned(),
            listener_id: id,
        });
        self.meta_listeners.push(RequestMetaListenerEntry {
            id,
            event,
            once,
            callback: Arc::new(listener),
        });
        id
    }

    fn add_listener(
        &mut self,
        name: Option<String>,
        listener: impl Fn(&RequestEvent) + Send + Sync + 'static,
        once: bool,
    ) -> RequestListenerId {
        let id = self.next_listener_id;
        self.next_listener_id = self.next_listener_id.wrapping_add(1);
        self.emit_meta(RequestMetaEvent::NewListener {
            event: name.clone().unwrap_or_default(),
            listener_id: id,
        });
        self.listeners.push(RequestListenerEntry {
            id,
            name,
            once,
            callback: Arc::new(listener),
        });
        id
    }

    pub fn off(&mut self, id: RequestListenerId) -> bool {
        self.remove_listener(id)
    }

    pub fn remove_listener(&mut self, id: RequestListenerId) -> bool {
        if let Some(index) = self.listeners.iter().position(|listener| listener.id == id) {
            let listener = self.listeners.remove(index);
            self.emit_meta(RequestMetaEvent::RemoveListener {
                event: listener.name.unwrap_or_default(),
                listener_id: listener.id,
            });
            return true;
        }
        if let Some(index) = self
            .meta_listeners
            .iter()
            .position(|listener| listener.id == id)
        {
            let listener = self.meta_listeners.remove(index);
            self.emit_meta(RequestMetaEvent::RemoveListener {
                event: meta_event_name(listener.event).to_owned(),
                listener_id: listener.id,
            });
            return true;
        }
        false
    }

    pub fn listener_count(&self) -> usize {
        self.listeners.len() + self.meta_listeners.len()
    }

    pub fn listener_count_named(&self, name: impl AsRef<str>) -> usize {
        self.listeners
            .iter()
            .filter(|listener| listener.name.as_deref() == Some(name.as_ref()))
            .count()
    }

    pub fn listeners(&self) -> Vec<RequestListenerId> {
        self.listeners
            .iter()
            .map(|listener| listener.id)
            .chain(self.meta_listeners.iter().map(|listener| listener.id))
            .collect::<Vec<_>>()
    }

    pub fn listeners_named(&self, name: impl AsRef<str>) -> Vec<RequestListenerId> {
        self.listeners
            .iter()
            .filter(|listener| listener.name.as_deref() == Some(name.as_ref()))
            .map(|listener| listener.id)
            .collect()
    }

    pub fn remove_all_listeners(&mut self, name: Option<&str>) -> usize {
        let mut ids = self
            .listeners
            .iter()
            .filter(|listener| name.is_none_or(|name| listener.name.as_deref() == Some(name)))
            .map(|listener| listener.id)
            .collect::<Vec<_>>();
        if name.is_none() {
            ids.extend(self.meta_listeners.iter().map(|listener| listener.id));
        }
        for id in ids.iter().rev() {
            self.remove_listener(*id);
        }
        ids.len()
    }

    pub fn cancel(&mut self) -> bool {
        if self.canceled {
            return false;
        }
        self.pending_events.clear();
        self.cancellation.cancel();
        self.canceled = true;
        self.state = RequestState::Canceled;
        self.emit_now("cancel", RequestEvent::Cancel);
        true
    }

    pub fn pause(&mut self) {
        if self.paused {
            return;
        }
        self.emit(RequestEvent::Pause);
        self.paused = true;
    }

    pub fn resume(&mut self) {
        if !self.paused {
            return;
        }
        self.paused = false;
        self.emit(RequestEvent::Resume);
        let pending = std::mem::take(&mut self.pending_events);
        for event in pending {
            self.emit(event);
        }
    }

    pub fn emit(&mut self, event: RequestEvent) {
        self.emit_checked(event);
    }

    pub fn emit_checked(&mut self, event: RequestEvent) -> bool {
        self.emit_result(event).is_ok_and(|count| count != 0)
    }

    pub fn emit_result(&mut self, event: RequestEvent) -> Result<usize, RequestEmitError> {
        let name = request_event_name(&event);
        self.emit_named_result(name, event)
    }

    pub fn emit_error_checked(&mut self, error: RequestError) -> Result<usize, RequestEmitError> {
        self.emit_result(RequestEvent::Error(error))
    }

    pub fn emit_named(&mut self, name: impl AsRef<str>, event: RequestEvent) {
        self.emit_named_checked(name, event);
    }

    pub fn emit_named_checked(&mut self, name: impl AsRef<str>, event: RequestEvent) -> bool {
        self.emit_named_result(name, event)
            .is_ok_and(|count| count != 0)
    }

    fn emit_named_result(
        &mut self,
        name: impl AsRef<str>,
        event: RequestEvent,
    ) -> Result<usize, RequestEmitError> {
        if self.canceled && suppress_after_cancel(&event) {
            return Ok(0);
        }
        if self.paused && !matches!(event, RequestEvent::Pause | RequestEvent::Resume) {
            self.pending_events.push(event);
            return Ok(0);
        }
        let is_error = matches!(event, RequestEvent::Error(_));
        let count = self.emit_named_count(name, event);
        if is_error && count == 0 {
            Err(RequestEmitError::UnhandledError)
        } else {
            Ok(count)
        }
    }

    fn emit_named_count(&mut self, name: impl AsRef<str>, event: RequestEvent) -> usize {
        self.emit_now(name.as_ref(), event) as usize
    }

    fn emit_now(&mut self, name: &str, event: RequestEvent) -> bool {
        let callbacks = self
            .listeners
            .iter()
            .filter(|listener| listener.name.is_none() || listener.name.as_deref() == Some(name))
            .map(|listener| {
                (
                    listener.id,
                    listener.once,
                    listener.name.clone().unwrap_or_else(|| name.to_owned()),
                    Arc::clone(&listener.callback),
                )
            })
            .collect::<Vec<_>>();
        for (id, once, listener_name, _) in &callbacks {
            if *once {
                self.listeners.retain(|listener| listener.id != *id);
                self.emit_meta(RequestMetaEvent::RemoveListener {
                    event: listener_name.clone(),
                    listener_id: *id,
                });
            }
        }
        for (_, _, _, callback) in callbacks.iter() {
            callback(&event);
        }
        !callbacks.is_empty()
    }

    fn emit_meta(&mut self, event: RequestMetaEvent) {
        let kind = match event {
            RequestMetaEvent::NewListener { .. } => RequestMetaEventKind::NewListener,
            RequestMetaEvent::RemoveListener { .. } => RequestMetaEventKind::RemoveListener,
        };
        let listeners = std::mem::take(&mut self.meta_listeners);
        let mut callbacks = Vec::with_capacity(listeners.len());
        for listener in listeners {
            let RequestMetaListenerEntry {
                id,
                event: listener_event,
                once,
                callback,
            } = listener;
            if listener_event == kind {
                if !once {
                    self.meta_listeners.push(RequestMetaListenerEntry {
                        id,
                        event: kind,
                        once,
                        callback: Arc::clone(&callback),
                    });
                }
                callbacks.push((id, listener_event, once, callback));
            } else {
                self.meta_listeners.push(RequestMetaListenerEntry {
                    id,
                    event: listener_event,
                    once,
                    callback: Arc::clone(&callback),
                });
            }
        }
        for (listener_id, listener_event, once, callback) in callbacks {
            if once {
                self.emit_meta(RequestMetaEvent::RemoveListener {
                    event: meta_event_name(listener_event).to_owned(),
                    listener_id,
                });
            }
            callback(&event);
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn begin(&mut self) -> Result<(), RequestError> {
        if self.canceled {
            return Err(RequestError::new("Canceled.", Some("ECANCEL".to_owned())));
        }
        if !matches!(
            self.state,
            RequestState::Created | RequestState::Completed | RequestState::Failed
        ) {
            return Err(RequestError::new(
                "request has already started",
                Some("EINVALIDSTATE".to_owned()),
            ));
        }
        self.rows.clear();
        self.row_count = 0;
        self.errors.clear();
        self.return_status = None;
        self.error = None;
        self.crypto_metadata_loaded = false;
        self.crypto_metadata = None;
        self.pending_events.clear();
        self.completion_notified = false;
        self.cancellation = RequestCancellationHandle {
            cancelled: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        };
        self.state = RequestState::Running;
        Ok(())
    }

    pub(crate) fn begin_prepare(&mut self) {
        self.error = None;
        self.completion_notified = false;
        self.preparing = true;
    }

    pub(crate) fn complete_prepare(&mut self) {
        if !self.preparing || self.completion_notified {
            return;
        }
        self.preparing = false;
        self.state = RequestState::Created;
        self.completion_notified = true;
        self.emit(RequestEvent::Prepared);
    }

    pub fn finish(&mut self) {
        if self.completion_notified
            || matches!(self.state, RequestState::Completed | RequestState::Failed)
        {
            return;
        }
        if self.preparing {
            self.preparing = false;
            self.state = RequestState::Created;
            self.completion_notified = true;
            if let Some(error) = self.error.clone() {
                self.emit(RequestEvent::Error(error));
            } else {
                self.emit(RequestEvent::Prepared);
            }
            return;
        }
        if self.state == RequestState::Canceled {
            let error = self
                .error
                .clone()
                .unwrap_or_else(|| RequestError::new("Canceled.", Some("ECANCEL".to_owned())));
            self.error = Some(error.clone());
            self.state = RequestState::Completed;
            self.completion_notified = true;
            if let Some(callback) = self.callback.as_mut() {
                callback(Err(error));
            }
            self.emit_now("requestCompleted", RequestEvent::RequestCompleted);
            return;
        }
        self.state = RequestState::Completed;
        if let Some(callback) = self.callback.as_mut() {
            if let Some(error) = self.error.clone() {
                callback(Err(error));
            } else {
                callback(Ok(RequestResult {
                    row_count: usize::try_from(self.row_count).unwrap_or(usize::MAX),
                    rows: if self.row_collection_on_completion {
                        self.rows.clone()
                    } else {
                        Vec::new()
                    },
                }));
            }
        }
        if self.canceled {
            self.emit_now("requestCompleted", RequestEvent::RequestCompleted);
        } else {
            self.emit(RequestEvent::RequestCompleted);
        }
        self.completion_notified = true;
    }

    pub fn fail(&mut self, error: RequestError) {
        if self.completion_notified
            || matches!(self.state, RequestState::Completed | RequestState::Failed)
        {
            return;
        }
        if self.preparing {
            self.fail_preparing(error);
            return;
        }
        if !(self.canceled && error.code.as_deref() == Some("ECANCEL")) {
            self.state = RequestState::Failed;
        }
        self.error = Some(error.clone());
        self.completion_notified = true;
        if let Some(callback) = self.callback.as_mut() {
            callback(Err(error));
        }
        if self.canceled {
            self.emit_now("requestCompleted", RequestEvent::RequestCompleted);
        } else {
            self.emit(RequestEvent::RequestCompleted);
        }
    }

    pub fn fail_preparing(&mut self, error: RequestError) {
        if !self.preparing || matches!(self.state, RequestState::Completed | RequestState::Failed) {
            return;
        }
        self.preparing = false;
        self.state = RequestState::Created;
        self.completion_notified = true;
        self.emit(RequestEvent::Error(error));
    }

    pub fn fail_validation(&mut self, error: RequestError) {
        if matches!(self.state, RequestState::Completed | RequestState::Failed) {
            return;
        }
        self.state = RequestState::Failed;
        self.error = Some(error.clone());
        self.completion_notified = true;
        if let Some(callback) = self.callback.as_mut() {
            callback(Err(error));
        }
        self.emit(RequestEvent::RequestCompleted);
    }
}

fn suppress_after_cancel(event: &RequestEvent) -> bool {
    matches!(
        event,
        RequestEvent::ColumnMetadata(_)
            | RequestEvent::NamedColumnMetadata(_)
            | RequestEvent::Row(_)
            | RequestEvent::NamedRow(_)
            | RequestEvent::Done { .. }
            | RequestEvent::DoneProc { .. }
            | RequestEvent::DoneInProc { .. }
            | RequestEvent::Error(_)
            | RequestEvent::ReturnValue { .. }
            | RequestEvent::Order(_)
    )
}

fn request_event_name(event: &RequestEvent) -> &'static str {
    match event {
        RequestEvent::ColumnMetadata(_) => "columnMetadata",
        RequestEvent::NamedColumnMetadata(_) => "columnMetadata",
        RequestEvent::Row(_) => "row",
        RequestEvent::NamedRow(_) => "row",
        RequestEvent::Done { .. } => "done",
        RequestEvent::DoneProc { .. } => "doneProc",
        RequestEvent::DoneInProc { .. } => "doneInProc",
        RequestEvent::Error(_) => "error",
        RequestEvent::RequestCompleted => "requestCompleted",
        RequestEvent::Cancel => "cancel",
        RequestEvent::Pause => "pause",
        RequestEvent::Resume => "resume",
        RequestEvent::ReturnValue { .. } => "returnValue",
        RequestEvent::Order(_) => "order",
        RequestEvent::Prepared => "prepared",
    }
}

fn meta_event_name(event: RequestMetaEventKind) -> &'static str {
    match event {
        RequestMetaEventKind::NewListener => "newListener",
        RequestMetaEventKind::RemoveListener => "removeListener",
    }
}

fn encode_typed_parameter(
    parameter_type: SqlParameterType,
    value: &Value,
    options: &ParameterOptions,
) -> Result<crate::value_codec::EncodedParameter, crate::value_codec::CodecError> {
    if matches!(
        parameter_type,
        SqlParameterType::VarChar | SqlParameterType::Char | SqlParameterType::Text
    ) {
        Ok(crate::value_codec::EncodedParameter {
            type_info: Vec::new(),
            length: Vec::new(),
            data: Vec::new(),
        })
    } else {
        parameter_type.encode(value, options)
    }
}
