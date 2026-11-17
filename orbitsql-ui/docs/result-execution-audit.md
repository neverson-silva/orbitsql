# Result execution audit

## Legacy full-result pipeline

The current path is:

```text
ResultGrid
  -> QueryWorkspaceScreen.runQuery
  -> executeQueryInContext
  -> Tauri invoke("execute_query_in_context")
  -> commands::execute_query_in_context
  -> application::connections::execute_query_in_context
  -> RuntimeConnection::execute
  -> PostgresqlRuntimeConnection or SqlServerRuntimeConnection
  -> QueryResult { columns, rows, row_count }
  -> Query tab state
  -> ResultGrid
```

`RuntimeConnection::execute` is the current application port. It returns a complete `QueryResult`, so it cannot represent a live result source or a subsequent fetch.

## Current materialization points

- PostgreSQL prepares metadata, then calls `sqlx::query(...).fetch_all(&pool)` in `src-tauri/src/backend/adapters/runtime.rs`.
- PostgreSQL converts every fetched row to `serde_json::Value` and collects all converted rows into `Vec<Vec<Value>>` before returning.
- MSSQL calls `tedious_rs::Connection::exec_sql_batch`, which returns all `StreamToken` values. The adapter then collects every row into `Vec<Vec<Value>>`.
- `QueryResult` owns the complete row array and crosses the Tauri command boundary in one response.
- `QueryWorkspaceScreen` stores the complete result in the query tab.
- `ResultGrid` filters the complete row array and renders every visible result row from that array. There is currently no virtualizer or incremental result store.

## Current lifecycle and cancellation

The connection registry owns one runtime connection per connection id. A query borrows the runtime connection for the duration of `execute`. The Tauri command wraps execution in `Abortable` and `QueryCancellationRegistry` aborts the Rust future for the connection id. There is no result-session identity, fetch-next operation, or independent result-session cleanup.

The existing cancellation path is therefore query-future cancellation, not a result-stream contract. PostgreSQL has no explicit adapter-level cancellation operation. The MSSQL driver has request cancellation and pause/resume primitives, but the current Orbit adapter uses the collecting batch API and does not expose them as an incremental result session.

## Capability matrix before result sessions

| Capability | PostgreSQL / sqlx 0.9 | MSSQL / tedious-rs workspace driver |
| --- | --- | --- |
| Driver row streaming API | Supported by sqlx `fetch` | Supported internally by the token reader and request events |
| Used by Orbit runtime adapter | Not supported: `fetch_all` is used | Not supported: `exec_sql_batch` is collected |
| Server cursor | Not used; requires separate implementation | Not used by current adapter |
| Client stream | Available through sqlx `BoxStream` | Available through the driver's incremental token reader |
| Fetch-size control | Not exposed by current adapter | Not exposed by current adapter |
| Cancellation | Supported with limitations through dropping/aborting the query future; no explicit result-session API | Supported with limitations through driver request cancellation; no Orbit result-session API |
| Multiple result sets | Not represented by `QueryResult` | Driver tokens can represent multiple result boundaries; adapter flattens rows |
| Metadata before all rows | Supported by statement description | Supported by `ColMetadata` tokens |
| LOB behavior | Eager conversion to JSON values in current adapter | Eager conversion to JSON values in current adapter |
| Connection ownership during result consumption | Pool connection is owned by the collecting future only | Mutex-protected connection is owned by the collecting future only |
| Backpressure | Not present at application/IPC level | Not present at application/IPC level |

## Acceptance gate for the backend phase

The frontend remains unchanged until both adapters can independently prove:

1. One execution creates a live result session.
2. Metadata and a bounded first batch are available without collecting the full result.
3. Two subsequent batches continue the same execution without re-running SQL.
4. Cancellation and early close terminate the source and release resources.
5. Session identity prevents batches from crossing executions.
6. A second independent session can progress concurrently.

The frontend result grid, incremental loading, virtualization, and prefetch were intentionally deferred until the backend acceptance phase passed.

The query workspace now uses the result-session path below. The legacy command remains available for non-grid commands such as transaction control and for compatibility with existing callers.

## Current query pipeline

```text
QueryWorkspaceScreen.runQuery
  -> startQuerySessionInContext
  -> Tauri invoke("start_query_session_in_context")
  -> ResultSessionRegistry
  -> RuntimeConnection::start_result_session
  -> metadata + bounded first QueryResultBatch
  -> ResultGrid virtual window
  -> fetchQuerySession(sessionId, 200)
  -> bounded next QueryResultBatch
```

The frontend keeps at most 10,000 loaded rows per result in phase one. Once that limit is reached, it explicitly closes the live session and shows that the display is capped. The grid renders a viewport-sized window with overscan and requests more rows from visible-range proximity, not from raw scroll events. Closing a query tab also closes its active result session.

## Backend result-session contract

The backend now has a separate live-session path alongside the legacy full-result path:

```text
start_query_session -> QueryResultSessionStart + session id
fetch_query_session -> QueryResultBatch
fetch_query_session -> QueryResultBatch
close_query_session / cancel_query_session -> release source
```

`QueryResultBatch` contains only the requested rows, `row_count`, `has_more`, and `total_fetched`. The session registry associates each session with its connection id and closes sessions before disconnect, reconnect, application exit, or connection replacement.

PostgreSQL uses a SQLx row stream and one-row lookahead to determine `has_more` without fetching the complete result. SQL Server uses the incremental token reader in `tedious-rs`, a bounded channel, request pause/resume, and a request cancellation handle. The physical SQL Server connection is removed from the runtime slot while the session is active and returned only after the request has finished or been cancelled.

The `tedious-node` comparison confirmed the required model: its request API exposes incremental `row`/`done` events, pause/resume, and cancellation. `tedious-rs` already had the equivalent parser and pause/cancel primitives internally; the new `exec_sql_batch_stream` API exposes them without collecting all `StreamToken` values. Orbit's SQL Server adapter consumes that API directly.

The SQL Server session path currently accepts one result set per execution. Multiple result sets return a typed `RESULT_MULTIPLE_SETS_UNSUPPORTED` error. This is deliberate until result-set boundaries and per-set metadata are represented in the domain contract.

The real-driver acceptance tests are in `tedious-rs/tests/integration/orbitsql_ui_result_sessions.rs` and are ignored by default. With `ORBITSQL_TEST_POSTGRES_*` or `ORBITSQL_TEST_SQLSERVER_*` configured, each test verifies one execution followed by 200 rows, 200 rows, 200 rows, ordered values without duplicates, early cancellation, connection reuse, and explicit close. The normal compile-only check is:

```text
cargo check -p orbitsql-ui --test orbitsql_ui_result_sessions
```

The live integration command remains environment-dependent and must be run before claiming the runtime acceptance gate is green:

```text
cargo test -p orbitsql-ui --test orbitsql_ui_result_sessions -- --ignored
```
