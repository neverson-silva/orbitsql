# ADR-003 TypeScript owns SQL intelligence

## Decisão

Parsing contextual, escopos, aliases, CTEs, tipos, candidatos, ranking e text edits são implementados em TypeScript puro.

## Consequências

O hot path é síncrono, local e testável sem Tauri, banco ou React.
