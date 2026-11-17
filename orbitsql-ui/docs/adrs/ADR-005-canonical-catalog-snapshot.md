# ADR-005 Canonical catalog snapshot

## Decisão

Rust e TypeScript compartilham um modelo serializável com schemas, objetos, colunas, relações, constraints, índices, funções, procedures e `schemaVersion`.

## Consequências

Metadata é transferida por snapshot versionado, nunca por linhas brutas específicas de um vendor.
