use std::collections::BTreeMap;
use std::time::Duration;

use crate::collation::Collation;
use crate::data_type::DataType;
use crate::parameter::{ParameterOptions, SqlParameterType};
use crate::payloads::{BulkLoadColumn, BulkLoadPayload, BulkLoadRow};
use crate::value_codec::{EncodedParameter, Value};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

#[path = "bulk_load_payload.rs"]
mod bulk_load_payload;

pub type BulkLoadListenerId = u64;
#[derive(Clone)]
pub struct BulkLoadCancellationHandle {
    canceled: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl BulkLoadCancellationHandle {
    pub fn cancel(&self) {
        if !self.canceled.swap(true, Ordering::AcqRel) {
            self.notify.notify_waiters();
        }
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::Acquire)
    }

    pub async fn canceled(&self) {
        let notified = self.notify.notified();
        if self.is_canceled() {
            return;
        }
        notified.await;
    }
}
type BulkLoadListener = Arc<dyn Fn(&BulkLoadEvent) + Send + Sync>;
type BulkLoadMetaListener = Arc<dyn Fn(&BulkLoadMetaEvent) + Send + Sync>;
type BulkLoadCompletionCallback = Box<dyn FnOnce(Result<usize, String>) + Send>;

fn null_encoded_parameter(type_info: &[u8]) -> EncodedParameter {
    let type_id = type_info.first().copied();
    let is_plp =
        matches!(type_id, Some(0xA5 | 0xA7 | 0xE7)) && type_info.get(1..3) == Some(&[0xFF, 0xFF]);
    let length = if matches!(type_id, Some(0x22 | 0x23 | 0x63)) {
        Vec::new()
    } else if is_plp {
        vec![0xFF; 8]
    } else if matches!(
        type_id,
        Some(0xA5 | 0xA7 | 0xAD | 0xAF | 0xAE | 0xE7 | 0xEF | 0xF3)
    ) {
        vec![0xFF, 0xFF]
    } else {
        vec![0]
    };
    EncodedParameter {
        type_info: type_info.to_vec(),
        length,
        data: if matches!(type_id, Some(0xF3)) {
            vec![0, 0]
        } else {
            Vec::new()
        },
    }
}

struct BulkLoadListenerEntry {
    id: BulkLoadListenerId,
    name: Option<String>,
    once: bool,
    callback: BulkLoadListener,
}

struct BulkLoadMetaListenerEntry {
    id: BulkLoadListenerId,
    event: BulkLoadMetaEventName,
    once: bool,
    callback: BulkLoadMetaListener,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BulkLoadMetaEventName {
    NewListener,
    RemoveListener,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BulkLoadEvent {
    Cancel,
    Error(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BulkLoadMetaEvent {
    NewListener {
        event: String,
        listener_id: BulkLoadListenerId,
    },
    RemoveListener {
        event: String,
        listener_id: BulkLoadListenerId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BulkLoadEmitError {
    UnhandledError,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BulkLoadOptions {
    pub check_constraints: bool,
    pub fire_triggers: bool,
    pub keep_nulls: bool,
    pub lock_table: bool,
    pub order: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulkLoadColumnDefinition {
    pub name: String,
    pub obj_name: String,
    pub declaration: String,
    pub type_info: Vec<u8>,
    pub nullable: bool,
    pub legacy_lob: bool,
}

pub struct BulkLoad {
    pub table: String,
    pub columns: Vec<BulkLoadColumnDefinition>,
    pub rows: Vec<BulkLoadRow>,
    pub options: BulkLoadOptions,
    pub canceled: bool,
    pub execution_started: bool,
    pub timeout: Option<Duration>,
    pub tds_version: String,
    typed_columns: Vec<Option<(SqlParameterType, ParameterOptions)>>,
    collation: Option<Collation>,
    listeners: Vec<BulkLoadListenerEntry>,
    meta_listeners: Vec<BulkLoadMetaListenerEntry>,
    next_listener_id: BulkLoadListenerId,
    nullable_unknown_columns: Vec<usize>,
    nullable_columns: Vec<Option<bool>>,
    completion_callback: Option<BulkLoadCompletionCallback>,
    completion_called: bool,
    cancellation: BulkLoadCancellationHandle,
}

impl BulkLoad {
    pub fn new(table: impl Into<String>, options: BulkLoadOptions) -> Result<Self, String> {
        let table = table.into();
        if options
            .order
            .iter()
            .any(|(_, direction)| direction != "ASC" && direction != "DESC")
        {
            return Err("bulk-load order direction must be ASC or DESC".to_owned());
        }
        Ok(Self {
            table,
            columns: Vec::new(),
            rows: Vec::new(),
            options,
            canceled: false,
            execution_started: false,
            timeout: None,
            tds_version: "7_4".to_owned(),
            typed_columns: Vec::new(),
            collation: None,
            listeners: Vec::new(),
            meta_listeners: Vec::new(),
            next_listener_id: 0,
            nullable_unknown_columns: Vec::new(),
            nullable_columns: Vec::new(),
            completion_callback: None,
            completion_called: false,
            cancellation: BulkLoadCancellationHandle {
                canceled: Arc::new(AtomicBool::new(false)),
                notify: Arc::new(Notify::new()),
            },
        })
    }

    pub fn new_with_callback(
        table: impl Into<String>,
        options: BulkLoadOptions,
        callback: impl FnOnce(Result<usize, String>) + Send + 'static,
    ) -> Result<Self, String> {
        let mut bulk_load = Self::new(table, options)?;
        bulk_load.completion_callback = Some(Box::new(callback));
        Ok(bulk_load)
    }

    pub fn set_completion_callback(
        &mut self,
        callback: impl FnOnce(Result<usize, String>) + Send + 'static,
    ) -> Result<(), String> {
        if self.completion_called {
            return Err("bulk-load completion has already been reported".to_owned());
        }
        if self.completion_callback.is_some() {
            return Err("bulk-load completion callback is already set".to_owned());
        }
        self.completion_callback = Some(Box::new(callback));
        Ok(())
    }

    pub fn complete(&mut self, row_count: usize) -> bool {
        self.finish(Ok(row_count))
    }

    pub fn fail(&mut self, error: impl Into<String>) -> bool {
        self.finish(Err(error.into()))
    }

    pub fn finish(&mut self, result: Result<usize, String>) -> bool {
        if self.completion_called {
            return false;
        }
        self.completion_called = true;
        if let Some(callback) = self.completion_callback.take() {
            callback(result);
        }
        true
    }

    pub fn begin_execution(&mut self) -> Result<(), String> {
        if self.execution_started {
            return Err("bulk-load execution has already started".to_owned());
        }
        if self.canceled {
            return Err("bulk load was canceled".to_owned());
        }
        self.execution_started = true;
        Ok(())
    }

    pub fn set_nullable_unknown_columns<I>(&mut self, columns: I) -> Result<(), String>
    where
        I: IntoIterator<Item = usize>,
    {
        let columns = columns.into_iter().collect::<Vec<_>>();
        if columns.iter().any(|index| *index >= self.columns.len()) {
            return Err("bulk nullable-unknown column index is out of range".to_owned());
        }
        self.nullable_unknown_columns = columns;
        for index in 0..self.columns.len() {
            self.nullable_columns[index] = if self.nullable_unknown_columns.contains(&index) {
                None
            } else {
                Some(self.columns[index].nullable)
            };
        }
        Ok(())
    }

    pub fn nullable_unknown_columns(&self) -> &[usize] {
        &self.nullable_unknown_columns
    }

    pub fn add_column(&mut self, column: BulkLoadColumnDefinition) -> Result<(), String> {
        let nullable = column.nullable;
        self.add_column_with_nullable(column, Some(nullable))
    }

    pub fn add_column_with_nullable(
        &mut self,
        column: BulkLoadColumnDefinition,
        nullable: Option<bool>,
    ) -> Result<(), String> {
        if !self.rows.is_empty() {
            return Err("cannot add bulk-load columns after the first row".to_owned());
        }
        if self.execution_started {
            return Err("cannot add bulk-load columns after execution starts".to_owned());
        }
        self.columns.push(column);
        self.typed_columns.push(None);
        let index = self.columns.len() - 1;
        self.nullable_columns.push(nullable);
        if nullable.is_none() && self.tds_version.as_str() >= "7_2" {
            self.nullable_unknown_columns.push(index);
        }
        Ok(())
    }

    pub fn add_typed_column(
        &mut self,
        name: impl Into<String>,
        data_type: DataType,
        options: ParameterOptions,
        nullable: bool,
    ) -> Result<(), String> {
        self.add_typed_column_with_nullable(name, data_type, options, Some(nullable))
    }

    pub fn add_typed_column_with_nullable(
        &mut self,
        name: impl Into<String>,
        data_type: DataType,
        options: ParameterOptions,
        nullable: Option<bool>,
    ) -> Result<(), String> {
        let name = name.into();
        self.add_typed_column_with_object_name_nullable(
            name.clone(),
            name,
            data_type,
            options,
            nullable,
        )
    }

    pub fn add_typed_column_with_object_name(
        &mut self,
        name: impl Into<String>,
        obj_name: impl Into<String>,
        data_type: DataType,
        options: ParameterOptions,
        nullable: bool,
    ) -> Result<(), String> {
        self.add_typed_column_with_object_name_nullable(
            name,
            obj_name,
            data_type,
            options,
            Some(nullable),
        )
    }

    pub fn add_typed_column_with_object_name_nullable(
        &mut self,
        name: impl Into<String>,
        obj_name: impl Into<String>,
        data_type: DataType,
        options: ParameterOptions,
        nullable: Option<bool>,
    ) -> Result<(), String> {
        let mut options = options;
        if self.collation.is_some() {
            options.collation = self.collation.clone();
        }
        if options.length.is_none() {
            options.length = data_type.maximum_length;
        }
        let value = Value::Null;
        let declaration = data_type
            .declaration(&value, &options, false)
            .map_err(|error| error.0)?;
        let encoded = data_type
            .encode(&value, &options)
            .map_err(|error| error.0)?;
        let legacy_lob = data_type.has_table_name
            || matches!(encoded.type_info.first(), Some(0x22 | 0x23 | 0x63));
        self.add_column_with_nullable(
            BulkLoadColumnDefinition {
                name: name.into(),
                obj_name: obj_name.into(),
                declaration,
                type_info: encoded.type_info,
                nullable: nullable.unwrap_or(true),
                legacy_lob,
            },
            nullable,
        )?;
        let slot = self
            .typed_columns
            .last_mut()
            .ok_or_else(|| "bulk-load typed column was not recorded".to_owned())?;
        *slot = Some(
            data_type
                .parameter_type
                .ok_or_else(|| "bulk-load type cannot encode values".to_owned())
                .map(|parameter_type| (parameter_type, options))?,
        );
        Ok(())
    }

    pub fn set_collation(&mut self, collation: Option<Collation>) {
        self.collation = collation.clone();
        for typed in self.typed_columns.iter_mut().flatten() {
            typed.1.collation = collation.clone();
        }
    }

    pub fn add_row(&mut self, values: Vec<EncodedParameter>) -> Result<(), String> {
        if self.execution_started {
            return Err("cannot add bulk-load rows after execution starts".to_owned());
        }
        let null_values = self
            .columns
            .iter()
            .map(|column| null_encoded_parameter(&column.type_info))
            .collect::<Vec<_>>();
        let legacy_lob = self
            .columns
            .iter()
            .map(|column| column.legacy_lob)
            .collect::<Vec<_>>();
        let values = bulk_load_payload::normalize_row(&legacy_lob, &null_values, values)?;
        self.rows.push(BulkLoadRow { values });
        Ok(())
    }

    pub fn add_value_row(&mut self, values: Vec<Value>) -> Result<(), String> {
        if self.execution_started {
            return Err("cannot add bulk-load rows after execution starts".to_owned());
        }
        if self.typed_columns.len() != self.columns.len()
            || self.typed_columns.iter().any(Option::is_none)
        {
            return Err("bulk-load row values require typed columns".to_owned());
        }
        let encoded = self.encode_value_row(&values)?;
        self.rows.push(BulkLoadRow { values: encoded });
        Ok(())
    }

    pub fn encode_value_row(&self, values: &[Value]) -> Result<Vec<EncodedParameter>, String> {
        if self.typed_columns.len() != self.columns.len()
            || self.typed_columns.iter().any(Option::is_none)
        {
            return Err("bulk-load row values require typed columns".to_owned());
        }
        let mut encoded = Vec::with_capacity(values.len());
        for (index, typed) in self.typed_columns.iter().enumerate() {
            let (parameter_type, options) = typed
                .as_ref()
                .ok_or_else(|| "bulk-load row values require typed columns".to_owned())?;
            let value = values.get(index).unwrap_or(&Value::Null);
            let encoded_value = parameter_type
                .encode(value, options)
                .map_err(|error| error.0)?;
            encoded.push(encoded_value);
        }
        let legacy_lob = self
            .columns
            .iter()
            .map(|column| column.legacy_lob)
            .collect::<Vec<_>>();
        let null_values = self
            .columns
            .iter()
            .map(|column| null_encoded_parameter(&column.type_info))
            .collect::<Vec<_>>();
        bulk_load_payload::normalize_row(&legacy_lob, &null_values, encoded)
    }

    pub fn add_object_row(&mut self, values: &BTreeMap<String, Value>) -> Result<(), String> {
        let row = self
            .columns
            .iter()
            .map(|column| values.get(&column.obj_name).cloned().unwrap_or(Value::Null))
            .collect();
        self.add_value_row(row)
    }

    pub fn get_bulk_insert_sql(&self) -> String {
        let columns = self
            .columns
            .iter()
            .map(|column| format!("[{}] {}", column.name, column.declaration))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "insert bulk {}({}){}",
            self.table,
            columns,
            self.get_options_sql()
        )
    }

    pub fn get_options_sql(&self) -> String {
        let mut options = Vec::new();
        if self.options.check_constraints {
            options.push("CHECK_CONSTRAINTS".to_owned());
        }
        if self.options.fire_triggers {
            options.push("FIRE_TRIGGERS".to_owned());
        }
        if self.options.keep_nulls {
            options.push("KEEP_NULLS".to_owned());
        }
        if self.options.lock_table {
            options.push("TABLOCK".to_owned());
        }
        if !self.options.order.is_empty() {
            let order = self
                .options
                .order
                .iter()
                .map(|(name, direction)| format!("{name} {direction}"))
                .collect::<Vec<_>>()
                .join(", ");
            options.push(format!("ORDER ({order})"));
        }
        if options.is_empty() {
            String::new()
        } else {
            format!(" WITH ({})", options.join(","))
        }
    }

    pub fn get_table_creation_sql(&self) -> String {
        let columns = self
            .columns
            .iter()
            .enumerate()
            .map(|column| {
                let (index, column) = column;
                let declaration = format!("[{}] {}", column.name, column.declaration);
                match self.nullable_columns.get(index).copied().flatten() {
                    Some(true) => format!("{declaration} NULL"),
                    Some(false) => format!("{declaration} NOT NULL"),
                    None => declaration,
                }
            })
            .collect::<Vec<_>>()
            .join(",\n");
        format!("CREATE TABLE {}(\n{}\n)", self.table, columns)
    }

    pub fn payload(&self) -> BulkLoadPayload {
        BulkLoadPayload {
            columns: self
                .columns
                .iter()
                .enumerate()
                .map(|(index, column)| BulkLoadColumn {
                    name: column.name.clone(),
                    table_name: self.table.clone(),
                    type_info: column.type_info.clone(),
                    nullable: self
                        .nullable_columns
                        .get(index)
                        .copied()
                        .flatten()
                        .unwrap_or(column.nullable)
                        && !self.nullable_unknown_columns.contains(&index),
                    legacy_lob: column.legacy_lob,
                })
                .collect(),
            rows: self.rows.clone(),
            tds_version: self.tds_version.clone(),
        }
    }

    pub fn get_col_metadata(&self) -> Result<Vec<u8>, String> {
        self.payload()
            .column_metadata_with_nullable_unknown(&self.nullable_unknown_columns)
    }

    pub fn get_col_metadata_with_nullable_unknown(
        &self,
        columns: &[usize],
    ) -> Result<Vec<u8>, String> {
        if columns.iter().any(|index| *index >= self.columns.len()) {
            return Err("bulk nullable-unknown column index is out of range".to_owned());
        }
        let mut payload = self.payload();
        for index in columns {
            payload.columns[*index].nullable = false;
        }
        payload.column_metadata_with_nullable_unknown(columns)
    }

    pub fn create_done_token(&self) -> Result<Vec<u8>, String> {
        let mut done = vec![0xFD, 0, 0, 0, 0];
        done.extend([0; 8]);
        if self.tds_version.as_str() < "7_2" {
            done.truncate(done.len() - 4);
        }
        Ok(done)
    }

    pub fn to_string(&self, indent: &str) -> String {
        format!("{indent}BulkLoad")
    }

    pub fn on(
        &mut self,
        listener: impl Fn(&BulkLoadEvent) + Send + Sync + 'static,
    ) -> BulkLoadListenerId {
        self.add_listener(None, listener, false)
    }

    pub fn once(
        &mut self,
        listener: impl Fn(&BulkLoadEvent) + Send + Sync + 'static,
    ) -> BulkLoadListenerId {
        self.add_listener(None, listener, true)
    }

    pub fn on_named(
        &mut self,
        event: impl Into<String>,
        listener: impl Fn(&BulkLoadEvent) + Send + Sync + 'static,
    ) -> BulkLoadListenerId {
        self.add_listener(Some(event.into()), listener, false)
    }

    pub fn once_named(
        &mut self,
        event: impl Into<String>,
        listener: impl Fn(&BulkLoadEvent) + Send + Sync + 'static,
    ) -> BulkLoadListenerId {
        self.add_listener(Some(event.into()), listener, true)
    }

    fn add_listener(
        &mut self,
        name: Option<String>,
        listener: impl Fn(&BulkLoadEvent) + Send + Sync + 'static,
        once: bool,
    ) -> BulkLoadListenerId {
        let id = self.next_listener_id;
        self.next_listener_id = self.next_listener_id.wrapping_add(1);
        if let Some(event) = name.as_deref() {
            self.emit_meta(BulkLoadMetaEvent::NewListener {
                event: event.to_owned(),
                listener_id: id,
            });
        }
        self.listeners.push(BulkLoadListenerEntry {
            id,
            name,
            once,
            callback: Arc::new(listener),
        });
        id
    }

    pub fn on_new_listener(
        &mut self,
        listener: impl Fn(&BulkLoadMetaEvent) + Send + Sync + 'static,
    ) -> BulkLoadListenerId {
        self.add_meta_listener(BulkLoadMetaEventName::NewListener, listener, false)
    }

    pub fn once_new_listener(
        &mut self,
        listener: impl Fn(&BulkLoadMetaEvent) + Send + Sync + 'static,
    ) -> BulkLoadListenerId {
        self.add_meta_listener(BulkLoadMetaEventName::NewListener, listener, true)
    }

    pub fn on_remove_listener(
        &mut self,
        listener: impl Fn(&BulkLoadMetaEvent) + Send + Sync + 'static,
    ) -> BulkLoadListenerId {
        self.add_meta_listener(BulkLoadMetaEventName::RemoveListener, listener, false)
    }

    pub fn once_remove_listener(
        &mut self,
        listener: impl Fn(&BulkLoadMetaEvent) + Send + Sync + 'static,
    ) -> BulkLoadListenerId {
        self.add_meta_listener(BulkLoadMetaEventName::RemoveListener, listener, true)
    }

    fn add_meta_listener(
        &mut self,
        event: BulkLoadMetaEventName,
        listener: impl Fn(&BulkLoadMetaEvent) + Send + Sync + 'static,
        once: bool,
    ) -> BulkLoadListenerId {
        let id = self.next_listener_id;
        self.next_listener_id = self.next_listener_id.wrapping_add(1);
        self.emit_meta(BulkLoadMetaEvent::NewListener {
            event: match event {
                BulkLoadMetaEventName::NewListener => "newListener",
                BulkLoadMetaEventName::RemoveListener => "removeListener",
            }
            .to_owned(),
            listener_id: id,
        });
        self.meta_listeners.push(BulkLoadMetaListenerEntry {
            id,
            event,
            once,
            callback: Arc::new(listener),
        });
        id
    }

    pub fn off(&mut self, id: BulkLoadListenerId) -> bool {
        self.remove_listener(id)
    }

    pub fn remove_listener(&mut self, id: BulkLoadListenerId) -> bool {
        if let Some(index) = self.listeners.iter().position(|listener| listener.id == id) {
            let listener = self.listeners.remove(index);
            if let Some(event) = listener.name {
                self.emit_meta(BulkLoadMetaEvent::RemoveListener {
                    event,
                    listener_id: listener.id,
                });
            }
            return true;
        }
        if let Some(index) = self
            .meta_listeners
            .iter()
            .position(|listener| listener.id == id)
        {
            let listener = self.meta_listeners.remove(index);
            self.emit_meta(BulkLoadMetaEvent::RemoveListener {
                event: match listener.event {
                    BulkLoadMetaEventName::NewListener => "newListener",
                    BulkLoadMetaEventName::RemoveListener => "removeListener",
                }
                .to_owned(),
                listener_id: listener.id,
            });
            return true;
        }
        false
    }

    pub fn remove_all_listeners(&mut self) -> usize {
        let mut ids = self
            .listeners
            .iter()
            .map(|listener| listener.id)
            .chain(self.meta_listeners.iter().map(|listener| listener.id))
            .collect::<Vec<_>>();
        ids.sort_unstable_by(|left, right| right.cmp(left));
        for id in &ids {
            self.remove_listener(*id);
        }
        ids.len()
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

    pub fn listeners(&self) -> Vec<BulkLoadListenerId> {
        let mut ids = self
            .listeners
            .iter()
            .map(|listener| listener.id)
            .chain(self.meta_listeners.iter().map(|listener| listener.id))
            .collect::<Vec<_>>();
        ids.sort_unstable();
        ids
    }

    pub fn cancel(&mut self) {
        if self.canceled {
            return;
        }
        self.canceled = true;
        self.cancellation.cancel();
        self.emit(BulkLoadEvent::Cancel);
    }

    pub fn cancellation_handle(&self) -> BulkLoadCancellationHandle {
        self.cancellation.clone()
    }

    pub fn emit(&mut self, event: BulkLoadEvent) {
        self.emit_checked(event);
    }

    pub fn emit_checked(&mut self, event: BulkLoadEvent) -> bool {
        self.emit_result(event).is_ok_and(|count| count != 0)
    }

    pub fn emit_result(&mut self, event: BulkLoadEvent) -> Result<usize, BulkLoadEmitError> {
        let is_error = matches!(event, BulkLoadEvent::Error(_));
        let count = self.emit_event(event);
        if is_error && count == 0 {
            Err(BulkLoadEmitError::UnhandledError)
        } else {
            Ok(count)
        }
    }

    pub fn emit_error_checked(
        &mut self,
        error: impl Into<String>,
    ) -> Result<usize, BulkLoadEmitError> {
        self.emit_result(BulkLoadEvent::Error(error.into()))
    }

    fn emit_event(&mut self, event: BulkLoadEvent) -> usize {
        let listeners = std::mem::take(&mut self.listeners);
        let event_name = match &event {
            BulkLoadEvent::Cancel => "cancel",
            BulkLoadEvent::Error(_) => "error",
        };
        let mut callbacks = Vec::with_capacity(listeners.len());
        for listener in listeners {
            let BulkLoadListenerEntry {
                id,
                name,
                once,
                callback,
            } = listener;
            let matches_event = name.is_none() || name.as_deref() == Some(event_name);
            if once && matches_event {
                self.emit_meta(BulkLoadMetaEvent::RemoveListener {
                    event: name.clone().unwrap_or_else(|| event_name.to_owned()),
                    listener_id: id,
                });
            } else {
                self.listeners.push(BulkLoadListenerEntry {
                    id,
                    name: name.clone(),
                    once,
                    callback: Arc::clone(&callback),
                });
            }
            if matches_event {
                callbacks.push((id, once, callback));
            }
        }
        let callback_count = callbacks.len();
        for (_, _, callback) in callbacks {
            callback(&event);
        }
        callback_count
    }

    fn emit_meta(&mut self, event: BulkLoadMetaEvent) {
        let listeners = std::mem::take(&mut self.meta_listeners);
        for listener in listeners {
            let BulkLoadMetaListenerEntry {
                id,
                event: listener_event,
                once,
                callback,
            } = listener;
            let event_name = match &event {
                BulkLoadMetaEvent::NewListener { .. } => BulkLoadMetaEventName::NewListener,
                BulkLoadMetaEvent::RemoveListener { .. } => BulkLoadMetaEventName::RemoveListener,
            };
            let matches_event = listener_event == event_name;
            if !once || !matches_event {
                self.meta_listeners.push(BulkLoadMetaListenerEntry {
                    id,
                    event: listener_event,
                    once,
                    callback: Arc::clone(&callback),
                });
            }
            if matches_event {
                if once {
                    self.emit_meta(BulkLoadMetaEvent::RemoveListener {
                        event: match listener_event {
                            BulkLoadMetaEventName::NewListener => "newListener",
                            BulkLoadMetaEventName::RemoveListener => "removeListener",
                        }
                        .to_owned(),
                        listener_id: id,
                    });
                }
                callback(&event);
            }
        }
    }

    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }

    pub fn set_tds_version(&mut self, tds_version: impl Into<String>) {
        self.tds_version = tds_version.into();
    }
}
