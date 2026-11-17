# ADR-002 Rust owns database facts

## Decisão

Drivers, introspecção, normalização, versionamento e ciclo de vida de conexão pertencem ao crate Tauri/Rust.

## Consequências

O frontend recebe apenas `CatalogSnapshot` canônico e não conhece tabelas de catálogo específicas de cada fornecedor.
