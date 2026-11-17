# ADR-009 Catalog versioning

## Decisão

Toda requisição carrega `documentVersion` e `catalogVersion`. Índices são reconstruídos para cada snapshot e resultados carregam suas versões.

## Consequências

Refresh de catálogo e edição de documento invalidam resultados antigos sem cache global compartilhado entre editores.
