use crate::backend::domain::connection::ConnectionOperationError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionTelemetry {
    pub sessions: Vec<DatabaseSession>,
    pub monitor_session_id: i32,
    pub captured_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSession {
    pub session_id: i32,
    pub status: String,
    pub request_status: Option<String>,
    pub database: Option<String>,
    pub login: Option<String>,
    pub original_login: Option<String>,
    pub host: Option<String>,
    pub application: Option<String>,
    pub client_interface: Option<String>,
    pub client_address: Option<String>,
    pub local_address: Option<String>,
    pub local_port: Option<i32>,
    pub login_time: Option<String>,
    pub command: Option<String>,
    pub duration_ms: String,
    pub cpu_time_ms: i64,
    pub reads: String,
    pub writes: String,
    pub logical_reads: String,
    pub row_count: String,
    pub wait_type: Option<String>,
    pub wait_time_ms: i64,
    pub wait_resource: Option<String>,
    pub last_wait_type: Option<String>,
    pub blocking_session_id: Option<i32>,
    pub blocker_session_id: Option<i32>,
    pub root_blocker_session_id: Option<i32>,
    pub is_blocking: bool,
    pub is_root_blocker: bool,
    pub blocked_session_count: usize,
    pub blocking_depth: usize,
    pub open_transaction_count: i32,
    pub transaction_isolation_level: i32,
    pub transaction_isolation_name: String,
    pub batch_text: Option<String>,
    pub current_statement: Option<String>,
    pub percent_complete: f64,
    pub estimated_completion_time_ms: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionKillResult {
    pub session_id: i32,
    pub message: String,
}

pub fn validate_kill_session_id(
    session_id: i32,
    monitor_session_id: i32,
) -> Result<(), ConnectionOperationError> {
    if session_id <= 0 {
        return Err(ConnectionOperationError::new(
            "SESSION_KILL_INVALID_ID",
            "Session id must be a positive integer",
            false,
        ));
    }
    if session_id == monitor_session_id {
        return Err(ConnectionOperationError::new(
            "SESSION_KILL_SELF",
            "The telemetry session cannot terminate itself",
            false,
        ));
    }
    Ok(())
}

pub fn extract_current_statement(
    batch: &str,
    start_offset: i64,
    end_offset: i64,
) -> Option<String> {
    if start_offset < 0 || end_offset < -1 {
        return None;
    }
    let utf16 = batch.encode_utf16().collect::<Vec<_>>();
    let start = usize::try_from(start_offset / 2).ok()?.min(utf16.len());
    let end = if end_offset == -1 {
        utf16.len()
    } else {
        usize::try_from(end_offset / 2).ok()?.min(utf16.len())
    };
    if end < start {
        return None;
    }
    String::from_utf16(&utf16[start..end])
        .ok()
        .map(|text| text.trim().to_owned())
}

pub fn transaction_isolation_name(level: i32) -> String {
    match level {
        0 => "Unspecified",
        1 => "Read uncommitted",
        2 => "Read committed",
        3 => "Repeatable read",
        4 => "Serializable",
        5 => "Snapshot",
        _ => "Unknown",
    }
    .to_owned()
}

pub fn annotate_blocking_graph(sessions: &mut [DatabaseSession]) {
    for session in sessions.iter_mut() {
        if session.blocking_session_id == Some(0)
            || session.blocking_session_id == Some(session.session_id)
        {
            session.blocking_session_id = None;
        }
    }
    let edges = sessions
        .iter()
        .filter_map(|session| {
            session
                .blocking_session_id
                .filter(|blocker| *blocker > 0 && *blocker != session.session_id)
                .map(|blocker| (session.session_id, blocker))
        })
        .collect::<HashMap<_, _>>();
    let blocked_counts =
        edges
            .values()
            .fold(HashMap::<i32, usize>::new(), |mut counts, blocker| {
                *counts.entry(*blocker).or_default() += 1;
                counts
            });
    for session in sessions.iter_mut() {
        session.blocked_session_count = blocked_counts
            .get(&session.session_id)
            .copied()
            .unwrap_or_default();
        session.is_blocking = session.blocked_session_count > 0;
        session.is_root_blocker = session.is_blocking && !edges.contains_key(&session.session_id);
        session.root_blocker_session_id = session.is_root_blocker.then_some(session.session_id);
        let Some(blocker) = edges.get(&session.session_id).copied() else {
            continue;
        };
        session.blocker_session_id = Some(blocker);
        let mut current = blocker;
        let mut visited = vec![session.session_id];
        let mut depth = 1;
        let mut root_blocker_session_id = None;
        loop {
            if visited.contains(&current) {
                break;
            }
            visited.push(current);
            if let Some(next) = edges.get(&current).copied() {
                if visited.contains(&next) {
                    break;
                }
                current = next;
                depth += 1;
            } else {
                root_blocker_session_id = Some(current);
                break;
            }
        }
        session.blocking_depth = depth;
        session.root_blocker_session_id = root_blocker_session_id;
    }
}
