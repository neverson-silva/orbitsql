# ADR-011 Hexagonal backend adapters

## Decisão

A comunicação de catálogo do backend Tauri é organizada em domínio, aplicação, ports e adapters. O caso de uso `RefreshCatalogUseCase` depende de `CatalogProviderFactory`, e a factory seleciona adapters concretos para PostgreSQL, MySQL e MariaDB.

## Fronteiras

O domínio mantém o contrato canônico. Os ports definem request, provider, factory e erros. Os adapters de driver encapsulam SQLx e normalização vendor-specific. O command Tauri apenas converte a chamada IPC para o caso de uso.

## Consequências

O engine TypeScript não conhece drivers. Um provider pode ser substituído por fake ou novo adapter sem alterar o caso de uso. SQL Server e NTLM não possuem adapter registrado nesta fase.
