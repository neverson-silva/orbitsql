# ADR-007 Scope graph

## Decisão

Bindings, CTEs, tabelas derivadas e subconsultas são representados por scopes com parent explícito, ranges e símbolos projetados.

## Consequências

Escopo interno pode consultar o externo para correlação, enquanto o externo nunca enxerga bindings internos.
