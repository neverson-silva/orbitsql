# Contrato de construção de símbolos

source_file: `src/connection.ts`
source_range: `1:1-1:10`
symbol: `Connection.connect`
rust_module: `src/connection.rs`
rust_symbol: `Connection::connect`
inputs: configuration
outputs: connection result
state: lifecycle state
calls: transport and login
branches: success and failure
errors: protocol and transport
events: connection lifecycle
resources: socket and timers
fixture: deterministic login fixture
test: external compatibility test
status: verified
divergence: ownership adaptation

Os bytes mínimos e os Overloads do TypeScript são registrados antes da implementação.
