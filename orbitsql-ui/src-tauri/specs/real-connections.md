# Real connection vertical slice

The connection flow is divided into non-sensitive profile data, an operating-system secret store, driver ports, concrete database adapters, an application orchestration layer, and Tauri commands.

`ConnectionProfileRecord` is persisted under the application data directory and contains no password. `OsSecretStore` stores the password in the platform credential service using the stable connection UUID as the credential identity. Secret store calls run outside the asynchronous runtime thread.

`RealConnectionDriverFactory` registers only PostgreSQL and SQL Server. PostgreSQL uses SQLx with a bounded pool. SQL Server uses `tedious-rs` and supports SQL Authentication and NTLM without requiring a domain. Both adapters perform a real handshake before returning a runtime connection.

`test_connection` validates and opens a real connection, closes it, returns structured diagnostic data, and does not persist anything. `save_connection` creates the UUID, persists the secret and profile, opens the runtime connection, and compensates profile and secret writes if connection startup fails. `connect_connection`, `disconnect_connection`, `delete_connection`, `rename_connection`, and `refresh_connection_catalog` operate on persisted records and the runtime registry.

`save_connection` and `connect_connection` return the real initial catalog snapshot after a bounded schema-only introspection. `expand_connection_node` performs a real, scoped schema query and caches the resulting objects by connection UUID and scope. `refresh_connection_catalog` performs the complete adapter introspection and invalidates the connection cache first. Cache entries expire after five minutes and are invalidated on reconnect, disconnect, delete, update, and explicit refresh. `execute_query` resolves the existing runtime session by connection UUID and executes user SQL through the registered PostgreSQL pool or SQL Server `tedious-rs` session. Query columns, rows, affected counts, and elapsed time are serialized without fixture data.

The frontend keeps the password only in the controlled connection draft. It is sent to the backend command when testing or saving and is never placed in the Zustand explorer store or returned in a saved profile. The sidebar hydrates connection metadata from the backend and delegates lifecycle and catalog refresh operations through IPC.

The connections directory and query workspace no longer import connection or result fixtures. They render persisted profiles, live connection state, real catalog metadata, and real query results; an empty state is shown before a connection or query exists.
