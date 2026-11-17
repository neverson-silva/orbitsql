use crate::bulk_load::{BulkLoad, BulkLoadEvent, BulkLoadListenerId};
use crate::connection::{Connection, ConnectionEvent, ConnectionListenerId, ConnectionState};
pub use crate::connector::ConnectorError;
pub use crate::errors::{
    ConnectionError, ErrorCause, InputError, NotEnoughDataError, RequestError,
};
use crate::payloads::BulkLoadPayload;
pub type NotEnoughData = NotEnoughDataError;
pub use crate::ntlm::NtlmError;
use crate::request::{Request, RequestEvent, RequestListenerId};
pub use crate::request::{RequestCancellationHandle, RequestEmitError, RequestMetaEvent};
pub use crate::token::handler::UnexpectedTokenError;
use crate::token::stream::StreamToken;
use crate::transaction::IsolationLevel;
pub use crate::transport::TransportError;
pub use crate::value_codec::CodecError;
use crate::value_codec::Value;
use futures_util::Stream;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};
use tokio::sync::oneshot;

pub const LIBRARY_NAME: &str = crate::library::NAME;
pub const EVENT_NEW_LISTENER: &str = "newListener";
pub const EVENT_REMOVE_LISTENER: &str = "removeListener";
pub const CONNECTION_EVENT_NAMES: &[&str] = &[
    "connect",
    "error",
    "end",
    "newListener",
    "removeListener",
    "rerouting",
    "retry",
    "rollbackTransaction",
    "charsetChange",
    "databaseChange",
    "languageChange",
    "infoMessage",
    "errorMessage",
    "resetConnection",
    "secure",
    "databaseMirroringPartner",
    "packetSizeChange",
    "collationChange",
    "debug",
];
pub const REQUEST_EVENT_NAMES: &[&str] = &[
    "columnMetadata",
    "row",
    "done",
    "doneProc",
    "doneInProc",
    "error",
    "requestCompleted",
    "cancel",
    "pause",
    "resume",
    "returnValue",
    "order",
    "prepared",
];
pub const BULK_LOAD_EVENT_NAMES: &[&str] = &["cancel", "error"];

pub const EVENT_ERROR: &str = "error";
pub const EVENT_END: &str = "end";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventLifecycle {
    Emitted,
    NewListener,
    RemoveListener,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventEmitterError {
    UnhandledError,
    Closed,
    AlreadyClosed,
}

pub struct EventNotification<'a, E> {
    pub name: &'a str,
    pub target_name: Option<&'a str>,
    pub payload: Option<&'a E>,
    pub listener_id: Option<u64>,
    pub lifecycle: EventLifecycle,
}

type EventCallback<E> = Arc<dyn for<'a> Fn(&EventNotification<'a, E>) + Send + Sync>;

type ConnectionFacadeListener = Arc<dyn Fn(&ConnectionEvent) + Send + Sync>;
type ConnectionFacadeListenerEntry = (u64, Option<String>, bool, ConnectionFacadeListener);
struct ConnectionFacadeListenerState {
    listeners: Vec<ConnectionFacadeListenerEntry>,
    pending_events: Vec<ConnectionEvent>,
}
type ConnectionFacadeListeners = Arc<Mutex<ConnectionFacadeListenerState>>;

fn dispatch_connection_facade_event(
    state: &ConnectionFacadeListeners,
    event: &ConnectionEvent,
    end_emitted: &AtomicBool,
) {
    if matches!(event, ConnectionEvent::End) && end_emitted.swap(true, Ordering::AcqRel) {
        return;
    }
    let name = connection_event_name(event);
    let mut state_guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let entries = state_guard
        .listeners
        .iter()
        .filter(|(_, event_name, _, _)| event_name.is_none() || event_name.as_deref() == Some(name))
        .map(|(id, _, once, callback)| (*id, *once, Arc::clone(callback)))
        .collect::<Vec<_>>();
    if entries.is_empty() {
        state_guard.pending_events.push(event.clone());
        return;
    }
    drop(state_guard);
    for (id, once, callback) in entries {
        if once {
            state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .listeners
                .retain(|(listener_id, _, _, _)| *listener_id != id);
        }
        callback(event);
    }
}
type ConnectionCompletion = Pin<Box<oneshot::Receiver<Result<(), ConnectionError>>>>;

struct EventListener<E> {
    id: u64,
    name: String,
    once: bool,
    callback: EventCallback<E>,
}

pub struct EventEmitter<E> {
    listeners: Vec<EventListener<E>>,
    next_listener_id: u64,
    closed: bool,
    error_event: String,
    end_event: String,
    new_listener_event: String,
    remove_listener_event: String,
}

impl<E> Default for EventEmitter<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> EventEmitter<E> {
    pub fn new() -> Self {
        Self::with_event_names(
            EVENT_ERROR,
            EVENT_END,
            EVENT_NEW_LISTENER,
            EVENT_REMOVE_LISTENER,
        )
    }

    pub fn with_event_names(
        error_event: impl Into<String>,
        end_event: impl Into<String>,
        new_listener_event: impl Into<String>,
        remove_listener_event: impl Into<String>,
    ) -> Self {
        Self {
            listeners: Vec::new(),
            next_listener_id: 0,
            closed: false,
            error_event: error_event.into(),
            end_event: end_event.into(),
            new_listener_event: new_listener_event.into(),
            remove_listener_event: remove_listener_event.into(),
        }
    }

    pub fn on<F>(&mut self, name: impl Into<String>, callback: F) -> u64
    where
        F: for<'a> Fn(&EventNotification<'a, E>) + Send + Sync + 'static,
    {
        self.add_listener(name.into(), Arc::new(callback), false)
    }

    pub fn once<F>(&mut self, name: impl Into<String>, callback: F) -> u64
    where
        F: for<'a> Fn(&EventNotification<'a, E>) + Send + Sync + 'static,
    {
        self.add_listener(name.into(), Arc::new(callback), true)
    }

    fn add_listener(&mut self, name: String, callback: EventCallback<E>, once: bool) -> u64 {
        let id = self.next_listener_id;
        self.next_listener_id = self.next_listener_id.wrapping_add(1);
        let meta_name = self.new_listener_event.clone();
        self.dispatch(
            &meta_name,
            Some(&name),
            None,
            EventLifecycle::NewListener,
            Some(id),
        );
        self.listeners.push(EventListener {
            id,
            name,
            once,
            callback,
        });
        id
    }

    pub fn off(&mut self, id: u64) -> bool {
        self.remove_listener(id)
    }

    pub fn remove_listener(&mut self, id: u64) -> bool {
        let Some(index) = self.listeners.iter().position(|listener| listener.id == id) else {
            return false;
        };
        let listener = self.listeners.remove(index);
        let meta_name = self.remove_listener_event.clone();
        self.dispatch(
            &meta_name,
            Some(&listener.name),
            None,
            EventLifecycle::RemoveListener,
            Some(listener.id),
        );
        true
    }

    pub fn remove_all_listeners(&mut self, name: Option<&str>) -> usize {
        let mut ids = self
            .listeners
            .iter()
            .filter(|listener| name.is_none_or(|value| listener.name == value))
            .map(|listener| listener.id)
            .collect::<Vec<_>>();
        ids.reverse();
        for id in &ids {
            self.remove_listener(*id);
        }
        ids.len()
    }

    pub fn listener_count(&self, name: Option<&str>) -> usize {
        self.listeners
            .iter()
            .filter(|listener| name.is_none_or(|value| listener.name == value))
            .count()
    }

    pub fn listeners(&self, name: Option<&str>) -> Vec<u64> {
        self.listeners
            .iter()
            .filter(|listener| name.is_none_or(|value| listener.name == value))
            .map(|listener| listener.id)
            .collect()
    }

    pub fn emit(&mut self, name: impl AsRef<str>, payload: E) -> Result<usize, EventEmitterError> {
        if self.closed {
            return Err(EventEmitterError::Closed);
        }
        let name = name.as_ref().to_owned();
        let count = self.dispatch(&name, None, Some(&payload), EventLifecycle::Emitted, None);
        if name == self.error_event && count == 0 {
            return Err(EventEmitterError::UnhandledError);
        }
        Ok(count)
    }

    pub fn emit_checked(&mut self, name: impl AsRef<str>, payload: E) -> bool {
        self.emit(name, payload).is_ok_and(|count| count != 0)
    }

    fn dispatch(
        &mut self,
        name: &str,
        target_name: Option<&str>,
        payload: Option<&E>,
        lifecycle: EventLifecycle,
        listener_id: Option<u64>,
    ) -> usize {
        let matching = self
            .listeners
            .iter()
            .filter(|listener| listener.name == name)
            .map(|listener| listener.id)
            .collect::<Vec<_>>();
        let mut callbacks = Vec::with_capacity(matching.len());
        for id in matching {
            if let Some(index) = self.listeners.iter().position(|listener| listener.id == id) {
                let listener = &self.listeners[index];
                callbacks.push((listener.id, listener.once, Arc::clone(&listener.callback)));
            }
        }
        for (id, once, callback) in &callbacks {
            if *once {
                self.remove_listener(*id);
            }
            let notification = EventNotification {
                name,
                target_name,
                payload,
                listener_id,
                lifecycle,
            };
            callback(&notification);
        }
        callbacks.len()
    }

    pub fn close(&mut self) -> Result<(), EventEmitterError> {
        if self.closed {
            return Err(EventEmitterError::AlreadyClosed);
        }
        self.closed = true;
        let end_name = self.end_event.clone();
        self.dispatch(&end_name, None, None, EventLifecycle::Closed, None);
        Ok(())
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

pub type NodeConnection = Connection;
pub type NodeRequest = Request;
pub type NodeBulkLoad = BulkLoad;
pub type NodeConnectionError = crate::errors::ConnectionError;
pub type NodeRequestError = crate::errors::RequestError;
pub type NodeInputError = crate::errors::InputError;
pub type NodeUnexpectedTokenError = crate::token::handler::UnexpectedTokenError;
pub type NodeNotEnoughDataError = crate::errors::NotEnoughDataError;
pub type NodeConnectionConfiguration = crate::connection::ConnectionConfig;
pub type NodeConnectionOptions = crate::connection::SessionOptions;
pub type NodeRequestOptions = crate::request::RequestOptions;
pub type NodeBulkLoadOptions = crate::bulk_load::BulkLoadOptions;
pub type NodeRequestCancellationHandle = crate::request::RequestCancellationHandle;
pub type NodeRequestMetaEvent = crate::request::RequestMetaEvent;

pub struct NodeConnectionFacade {
    inner: Arc<Mutex<Option<Connection>>>,
    facade_state: Arc<Mutex<ConnectionState>>,
    connection_config: crate::connection::ConnectionConfig,
    close_handle: crate::connection::ConnectionCloseHandle,
    completion: Option<ConnectionCompletion>,
    listeners: ConnectionFacadeListeners,
    listener_start_signal: Arc<AtomicBool>,
    end_emitted: Arc<AtomicBool>,
    next_listener_id: u64,
}

#[allow(clippy::await_holding_lock)]
impl NodeConnectionFacade {
    pub(crate) fn start<F>(config: crate::connection::ConnectionConfig, callback: Option<F>) -> Self
    where
        F: FnOnce(Option<ConnectionError>) + Send + 'static,
    {
        let connection_config = config.clone();
        let mut connection = Connection::new(config);
        let close_handle = connection.close_handle();
        let listeners: ConnectionFacadeListeners =
            Arc::new(Mutex::new(ConnectionFacadeListenerState {
                listeners: Vec::new(),
                pending_events: Vec::new(),
            }));
        let listener_start_signal = Arc::new(AtomicBool::new(false));
        let end_emitted = Arc::new(AtomicBool::new(false));
        let forwarded_listeners = Arc::clone(&listeners);
        let forwarded_end_emitted = Arc::clone(&end_emitted);
        connection.on(move |event| {
            dispatch_connection_facade_event(&forwarded_listeners, event, &forwarded_end_emitted);
        });
        if let Some(callback) = callback {
            let callback = Arc::new(Mutex::new(Some(callback)));
            let connection_callback = Arc::clone(&callback);
            connection.once_named("connect", move |event| {
                let error = match event {
                    ConnectionEvent::Connect => None,
                    ConnectionEvent::ConnectError(error) => Some(error.clone()),
                    _ => return,
                };
                if let Some(callback) = connection_callback
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take()
                {
                    callback(error);
                }
            });
        }
        let inner = Arc::new(Mutex::new(Some(connection)));
        let facade_state = Arc::new(Mutex::new(ConnectionState::Connecting));
        let task_inner = Arc::clone(&inner);
        let task_state = Arc::clone(&facade_state);
        let task_listener_start_signal = Arc::clone(&listener_start_signal);
        let (sender, receiver) = oneshot::channel();
        std::thread::Builder::new()
            .name("tedious-rs-connect".to_owned())
            .spawn(move || {
                let start_deadline =
                    std::time::Instant::now() + std::time::Duration::from_millis(1);
                while !task_listener_start_signal.load(Ordering::Acquire)
                    && std::time::Instant::now() < start_deadline
                {
                    std::thread::yield_now();
                }
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = sender.send(Err(ConnectionError::new(
                            error.to_string(),
                            Some("ERUNTIME".to_owned()),
                        )));
                        return;
                    }
                };
                let mut connection = task_inner
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .take()
                    .expect("connection task owner");
                let result = runtime.block_on(async { connection.connect().await });
                *task_state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = if result.is_ok() {
                    ConnectionState::LoggedIn
                } else {
                    ConnectionState::Final
                };
                *task_inner
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(connection);
                drop(task_inner);
                let _ = sender.send(result);
            })
            .expect("failed to start connection task");
        Self {
            inner,
            facade_state,
            connection_config,
            close_handle,
            completion: Some(Box::pin(receiver)),
            listeners,
            listener_start_signal,
            end_emitted,
            next_listener_id: 0,
        }
    }

    pub fn on_named<F>(
        &mut self,
        event_name: impl Into<String>,
        listener: F,
    ) -> ConnectionListenerId
    where
        F: Fn(&ConnectionEvent) + Send + Sync + 'static,
    {
        self.add_facade_listener(Some(event_name.into()), listener, false)
    }

    pub fn once_named<F>(
        &mut self,
        event_name: impl Into<String>,
        listener: F,
    ) -> ConnectionListenerId
    where
        F: Fn(&ConnectionEvent) + Send + Sync + 'static,
    {
        self.add_facade_listener(Some(event_name.into()), listener, true)
    }

    pub fn on<F>(&mut self, listener: F) -> ConnectionListenerId
    where
        F: Fn(&ConnectionEvent) + Send + Sync + 'static,
    {
        self.add_facade_listener(None, listener, false)
    }

    fn add_facade_listener<F>(
        &mut self,
        event_name: Option<String>,
        listener: F,
        once: bool,
    ) -> ConnectionListenerId
    where
        F: Fn(&ConnectionEvent) + Send + Sync + 'static,
    {
        let id = self.next_listener_id;
        self.next_listener_id = self.next_listener_id.wrapping_add(1);
        let callback: ConnectionFacadeListener = Arc::new(listener);
        let replay = {
            let mut state = self
                .listeners
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state
                .listeners
                .push((id, event_name.clone(), once, Arc::clone(&callback)));
            let mut replay = Vec::new();
            let mut retained = Vec::with_capacity(state.pending_events.len());
            for event in state.pending_events.drain(..) {
                let matches = event_name
                    .as_deref()
                    .is_none_or(|name| connection_event_name(&event) == name);
                if matches && (!once || replay.is_empty()) {
                    replay.push(event);
                } else {
                    retained.push(event);
                }
            }
            state.pending_events = retained;
            replay
        };
        if once && !replay.is_empty() {
            self.listeners
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .listeners
                .retain(|(listener_id, _, _, _)| *listener_id != id);
        }
        for event in replay {
            callback(&event);
        }
        self.listener_start_signal.store(true, Ordering::Release);
        id
    }

    pub fn off(&mut self, id: ConnectionListenerId) -> bool {
        let mut state = self
            .listeners
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let length = state.listeners.len();
        state
            .listeners
            .retain(|(listener_id, _, _, _)| *listener_id != id);
        state.listeners.len() != length
    }

    pub fn close(&self) {
        self.close_handle.close();
        if let Ok(mut inner) = self.inner.lock()
            && let Some(connection) = inner.as_mut()
        {
            connection.close_now();
        }
        *self
            .facade_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = ConnectionState::Final;
        dispatch_connection_facade_event(&self.listeners, &ConnectionEvent::End, &self.end_emitted);
    }

    pub fn is_closed(&self) -> bool {
        self.close_handle.is_closed()
    }

    pub fn state(&self) -> ConnectionState {
        *self
            .facade_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn config(&self) -> crate::connection::ConnectionConfig {
        self.connection_config.clone()
    }

    async fn ready(&mut self) -> Result<(), ConnectionError> {
        if let Some(completion) = self.completion.take() {
            match completion.await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(error)) => Err(error),
                Err(_) => Err(ConnectionError::new(
                    "connection task was cancelled",
                    Some("ECANCELLED".to_owned()),
                )),
            }
        } else {
            Ok(())
        }
    }

    async fn lock_connection(
        &mut self,
    ) -> Result<MutexGuard<'_, Option<Connection>>, ConnectionError> {
        self.ready().await?;
        Ok(self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()))
    }

    async fn take_connection(&mut self) -> Result<Connection, ConnectionError> {
        self.ready().await?;
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
            .ok_or_else(|| {
                ConnectionError::new(
                    "connection facade is still in use",
                    Some("EINUSE".to_owned()),
                )
            })
    }

    fn put_connection(&self, connection: Connection) {
        *self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(connection);
    }

    pub async fn exec_sql_batch(
        &mut self,
        sql_text: impl Into<String>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let sql_text = sql_text.into();
        let mut connection = self.take_connection().await?;
        let result = connection.exec_sql_batch(sql_text).await;
        self.put_connection(connection);
        result
    }

    pub async fn exec_sql_batch_request(
        &mut self,
        request: &mut Request,
    ) -> Result<(), ConnectionError> {
        let mut guard = self.lock_connection().await?;
        let connection = guard.as_mut().ok_or_else(|| {
            ConnectionError::new(
                "connection facade is still in use",
                Some("EINUSE".to_owned()),
            )
        })?;
        connection.exec_sql_batch_request(request).await
    }

    pub async fn exec_sql(&mut self, request: &mut Request) -> Result<(), ConnectionError> {
        let mut guard = self.lock_connection().await?;
        let connection = guard.as_mut().ok_or_else(|| {
            ConnectionError::new(
                "connection facade is still in use",
                Some("EINUSE".to_owned()),
            )
        })?;
        connection.exec_sql(request).await
    }

    pub async fn call_procedure(&mut self, request: &mut Request) -> Result<(), ConnectionError> {
        let mut guard = self.lock_connection().await?;
        let connection = guard.as_mut().ok_or_else(|| {
            ConnectionError::new(
                "connection facade is still in use",
                Some("EINUSE".to_owned()),
            )
        })?;
        connection.call_procedure(request).await
    }

    pub async fn prepare(&mut self, request: &mut Request) -> Result<(), ConnectionError> {
        let mut guard = self.lock_connection().await?;
        let connection = guard.as_mut().ok_or_else(|| {
            ConnectionError::new(
                "connection facade is still in use",
                Some("EINUSE".to_owned()),
            )
        })?;
        connection.prepare(request).await
    }

    pub async fn execute(
        &mut self,
        request: &mut Request,
        values: Option<&[(String, Value)]>,
    ) -> Result<(), ConnectionError> {
        let mut guard = self.lock_connection().await?;
        let connection = guard.as_mut().ok_or_else(|| {
            ConnectionError::new(
                "connection facade is still in use",
                Some("EINUSE".to_owned()),
            )
        })?;
        connection.execute(request, values).await
    }

    pub async fn unprepare(&mut self, request: &mut Request) -> Result<(), ConnectionError> {
        let mut guard = self.lock_connection().await?;
        let connection = guard.as_mut().ok_or_else(|| {
            ConnectionError::new(
                "connection facade is still in use",
                Some("EINUSE".to_owned()),
            )
        })?;
        connection.unprepare(request).await
    }

    pub async fn reset(&mut self) -> Result<Vec<StreamToken>, ConnectionError> {
        let mut connection = self.take_connection().await?;
        let result = connection.reset().await;
        self.put_connection(connection);
        result
    }

    pub fn new_bulk_load(
        &self,
        table: impl Into<String>,
        options: crate::bulk_load::BulkLoadOptions,
    ) -> Result<BulkLoad, String> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map_or_else(
                || Err("connection facade is still in use".to_owned()),
                |connection| connection.new_bulk_load(table, options),
            )
    }

    pub async fn exec_bulk_load(
        &mut self,
        payload: &BulkLoadPayload,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let mut guard = self.lock_connection().await?;
        let connection = guard.as_mut().ok_or_else(|| {
            ConnectionError::new(
                "connection facade is still in use",
                Some("EINUSE".to_owned()),
            )
        })?;
        connection.exec_bulk_load(payload).await
    }

    pub async fn exec_bulk_load_object(
        &mut self,
        bulk_load: &mut BulkLoad,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let mut guard = self.lock_connection().await?;
        let connection = guard.as_mut().ok_or_else(|| {
            ConnectionError::new(
                "connection facade is still in use",
                Some("EINUSE".to_owned()),
            )
        })?;
        connection.exec_bulk_load_object(bulk_load).await
    }

    pub async fn exec_bulk_load_values<I>(
        &mut self,
        bulk_load: &mut BulkLoad,
        rows: I,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        I: IntoIterator<Item = Vec<Value>>,
    {
        let mut guard = self.lock_connection().await?;
        let connection = guard.as_mut().ok_or_else(|| {
            ConnectionError::new(
                "connection facade is still in use",
                Some("EINUSE".to_owned()),
            )
        })?;
        connection.exec_bulk_load_values(bulk_load, rows).await
    }

    pub async fn exec_bulk_load_objects<I>(
        &mut self,
        bulk_load: &mut BulkLoad,
        rows: I,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        I: IntoIterator<Item = std::collections::BTreeMap<String, Value>>,
    {
        let mut guard = self.lock_connection().await?;
        let connection = guard.as_mut().ok_or_else(|| {
            ConnectionError::new(
                "connection facade is still in use",
                Some("EINUSE".to_owned()),
            )
        })?;
        connection.exec_bulk_load_objects(bulk_load, rows).await
    }

    pub async fn exec_bulk_load_value_stream<S>(
        &mut self,
        bulk_load: &mut BulkLoad,
        rows: S,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        S: Stream<Item = Vec<Value>>,
    {
        let mut guard = self.lock_connection().await?;
        let connection = guard.as_mut().ok_or_else(|| {
            ConnectionError::new(
                "connection facade is still in use",
                Some("EINUSE".to_owned()),
            )
        })?;
        connection
            .exec_bulk_load_value_stream(bulk_load, rows)
            .await
    }

    pub async fn exec_bulk_load_object_stream<S>(
        &mut self,
        bulk_load: &mut BulkLoad,
        rows: S,
    ) -> Result<Vec<StreamToken>, ConnectionError>
    where
        S: Stream<Item = std::collections::BTreeMap<String, Value>>,
    {
        let mut guard = self.lock_connection().await?;
        let connection = guard.as_mut().ok_or_else(|| {
            ConnectionError::new(
                "connection facade is still in use",
                Some("EINUSE".to_owned()),
            )
        })?;
        connection
            .exec_bulk_load_object_stream(bulk_load, rows)
            .await
    }

    pub async fn begin_transaction(
        &mut self,
        name: impl Into<String>,
        isolation_level: IsolationLevel,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let name = name.into();
        let mut connection = self.take_connection().await?;
        let result = connection.begin_transaction(name, isolation_level).await;
        self.put_connection(connection);
        result
    }

    pub async fn begin_transaction_default(
        &mut self,
        name: impl Into<String>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let name = name.into();
        let mut connection = self.take_connection().await?;
        let result = connection.begin_transaction_default(name).await;
        self.put_connection(connection);
        result
    }

    pub async fn commit_transaction(
        &mut self,
        name: impl Into<String>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let name = name.into();
        let mut connection = self.take_connection().await?;
        let result = connection.commit_transaction(name).await;
        self.put_connection(connection);
        result
    }

    pub async fn rollback_transaction(
        &mut self,
        name: impl Into<String>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let name = name.into();
        let mut connection = self.take_connection().await?;
        let result = connection.rollback_transaction(name).await;
        self.put_connection(connection);
        result
    }

    pub async fn save_transaction(
        &mut self,
        name: impl Into<String>,
    ) -> Result<Vec<StreamToken>, ConnectionError> {
        let name = name.into();
        let mut connection = self.take_connection().await?;
        let result = connection.save_transaction(name).await;
        self.put_connection(connection);
        result
    }

    pub fn cancel(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .is_some_and(Connection::cancel)
    }
}

fn connection_event_name(event: &ConnectionEvent) -> &'static str {
    match event {
        ConnectionEvent::Connect | ConnectionEvent::ConnectError(_) => "connect",
        ConnectionEvent::Error(_) => "error",
        ConnectionEvent::End => "end",
        ConnectionEvent::NewListener { .. } => "newListener",
        ConnectionEvent::RemoveListener { .. } => "removeListener",
        ConnectionEvent::Rerouting => "rerouting",
        ConnectionEvent::Retry => "retry",
        ConnectionEvent::RollbackTransaction => "rollbackTransaction",
        ConnectionEvent::CharsetChange(_) => "charsetChange",
        ConnectionEvent::DatabaseChange(_) => "databaseChange",
        ConnectionEvent::LanguageChange(_) => "languageChange",
        ConnectionEvent::InfoMessage(_) => "infoMessage",
        ConnectionEvent::ErrorMessage(_) => "errorMessage",
        ConnectionEvent::ResetConnection => "resetConnection",
        ConnectionEvent::Secure => "secure",
        ConnectionEvent::DatabaseMirroringPartner(_) => "databaseMirroringPartner",
        ConnectionEvent::PacketSizeChange(_) => "packetSizeChange",
        ConnectionEvent::CollationChange(_) => "collationChange",
        ConnectionEvent::Debug(_) => "debug",
    }
}

impl Future for NodeConnectionFacade {
    type Output = Result<Connection, ConnectionError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let completion = self
            .completion
            .as_mut()
            .expect("connection completion owner");
        match completion.as_mut().poll(context) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(result)) => {
                self.completion.take();
                let inner = Arc::clone(&self.inner);
                match Arc::try_unwrap(inner) {
                    Ok(inner) => Poll::Ready(Ok(inner
                        .into_inner()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .expect("connection task owner"))),
                    Err(inner) => {
                        self.inner = inner;
                        Poll::Ready(result.and_then(|_| {
                            Err(ConnectionError::new(
                                "connection facade is still in use",
                                Some("EINUSE".to_owned()),
                            ))
                        }))
                    }
                }
            }
            Poll::Ready(Err(_)) => Poll::Ready(Err(ConnectionError::new(
                "connection task was cancelled",
                Some("ECANCELLED".to_owned()),
            ))),
        }
    }
}

impl Drop for NodeConnectionFacade {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionEventName {
    Connect,
    Error,
    End,
    NewListener,
    RemoveListener,
    Rerouting,
    Retry,
    RollbackTransaction,
    CharsetChange,
    DatabaseChange,
    LanguageChange,
    InfoMessage,
    ErrorMessage,
    ResetConnection,
    Secure,
    DatabaseMirroringPartner,
    PacketSizeChange,
    CollationChange,
    Debug,
}

impl ConnectionEventName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Connect => "connect",
            Self::Error => "error",
            Self::End => "end",
            Self::NewListener => "newListener",
            Self::RemoveListener => "removeListener",
            Self::Rerouting => "rerouting",
            Self::Retry => "retry",
            Self::RollbackTransaction => "rollbackTransaction",
            Self::CharsetChange => "charsetChange",
            Self::DatabaseChange => "databaseChange",
            Self::LanguageChange => "languageChange",
            Self::InfoMessage => "infoMessage",
            Self::ErrorMessage => "errorMessage",
            Self::ResetConnection => "resetConnection",
            Self::Secure => "secure",
            Self::DatabaseMirroringPartner => "databaseMirroringPartner",
            Self::PacketSizeChange => "packetSizeChange",
            Self::CollationChange => "collationChange",
            Self::Debug => "debug",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestEventName {
    ColumnMetadata,
    Row,
    NamedRow,
    Done,
    DoneProc,
    DoneInProc,
    Error,
    RequestCompleted,
    Cancel,
    Pause,
    Resume,
    ReturnValue,
    Order,
    Prepared,
}

impl RequestEventName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ColumnMetadata => "columnMetadata",
            Self::Row => "row",
            Self::NamedRow => "row",
            Self::Done => "done",
            Self::DoneProc => "doneProc",
            Self::DoneInProc => "doneInProc",
            Self::Error => "error",
            Self::RequestCompleted => "requestCompleted",
            Self::Cancel => "cancel",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::ReturnValue => "returnValue",
            Self::Order => "order",
            Self::Prepared => "prepared",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BulkLoadEventName {
    Cancel,
    Error,
}

impl BulkLoadEventName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cancel => "cancel",
            Self::Error => "error",
        }
    }
}

pub trait ConnectionEventEmitter {
    fn on_event<F>(&mut self, listener: F) -> ConnectionListenerId
    where
        F: Fn(&ConnectionEvent) + Send + Sync + 'static;

    fn once_event<F>(&mut self, listener: F) -> ConnectionListenerId
    where
        F: Fn(&ConnectionEvent) + Send + Sync + 'static;

    fn off_event(&mut self, id: ConnectionListenerId) -> bool;
    fn remove_all_event_listeners(&mut self) -> usize;
    fn emit_event_checked(&self, event: ConnectionEvent) -> bool;
}

impl ConnectionEventEmitter for Connection {
    fn on_event<F>(&mut self, listener: F) -> ConnectionListenerId
    where
        F: Fn(&ConnectionEvent) + Send + Sync + 'static,
    {
        self.on(listener)
    }

    fn once_event<F>(&mut self, listener: F) -> ConnectionListenerId
    where
        F: Fn(&ConnectionEvent) + Send + Sync + 'static,
    {
        self.once(listener)
    }

    fn off_event(&mut self, id: ConnectionListenerId) -> bool {
        self.off(id)
    }

    fn remove_all_event_listeners(&mut self) -> usize {
        let ids = self.listeners();
        let count = ids.len();
        for id in ids {
            self.remove_listener(id);
        }
        count
    }

    fn emit_event_checked(&self, event: ConnectionEvent) -> bool {
        let has_listeners = self.listener_count() != 0;
        self.emit(event);
        has_listeners
    }
}

pub trait RequestEventEmitter {
    fn on_event<F>(&mut self, listener: F) -> RequestListenerId
    where
        F: Fn(&RequestEvent) + Send + Sync + 'static;

    fn once_event<F>(&mut self, listener: F) -> RequestListenerId
    where
        F: Fn(&RequestEvent) + Send + Sync + 'static;

    fn off_event(&mut self, id: RequestListenerId) -> bool;
    fn remove_all_event_listeners(&mut self) -> usize;
    fn emit_event_checked(&mut self, event: RequestEvent) -> bool;
}

impl RequestEventEmitter for Request {
    fn on_event<F>(&mut self, listener: F) -> RequestListenerId
    where
        F: Fn(&RequestEvent) + Send + Sync + 'static,
    {
        self.on(listener)
    }

    fn once_event<F>(&mut self, listener: F) -> RequestListenerId
    where
        F: Fn(&RequestEvent) + Send + Sync + 'static,
    {
        self.once(listener)
    }

    fn off_event(&mut self, id: RequestListenerId) -> bool {
        self.off(id)
    }

    fn remove_all_event_listeners(&mut self) -> usize {
        let ids = self.listeners();
        let count = ids.len();
        for id in ids {
            self.remove_listener(id);
        }
        count
    }

    fn emit_event_checked(&mut self, event: RequestEvent) -> bool {
        self.emit_checked(event)
    }
}

pub trait BulkLoadEventEmitter {
    fn on_event<F>(&mut self, listener: F) -> BulkLoadListenerId
    where
        F: Fn(&BulkLoadEvent) + Send + Sync + 'static;

    fn once_event<F>(&mut self, listener: F) -> BulkLoadListenerId
    where
        F: Fn(&BulkLoadEvent) + Send + Sync + 'static;

    fn off_event(&mut self, id: BulkLoadListenerId) -> bool;
    fn remove_all_event_listeners(&mut self) -> usize;
    fn emit_event_checked(&mut self, event: BulkLoadEvent) -> bool;
}

impl BulkLoadEventEmitter for BulkLoad {
    fn on_event<F>(&mut self, listener: F) -> BulkLoadListenerId
    where
        F: Fn(&BulkLoadEvent) + Send + Sync + 'static,
    {
        self.on(listener)
    }

    fn once_event<F>(&mut self, listener: F) -> BulkLoadListenerId
    where
        F: Fn(&BulkLoadEvent) + Send + Sync + 'static,
    {
        self.once(listener)
    }

    fn off_event(&mut self, id: BulkLoadListenerId) -> bool {
        self.off(id)
    }

    fn remove_all_event_listeners(&mut self) -> usize {
        let ids = self.listeners();
        let count = ids.len();
        for id in ids {
            self.remove_listener(id);
        }
        count
    }

    fn emit_event_checked(&mut self, event: BulkLoadEvent) -> bool {
        self.emit_checked(event)
    }
}
