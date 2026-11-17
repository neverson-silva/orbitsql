# ADR-006 Dialect registry

## Decisão

Quoting, keywords, funções, operadores, parâmetros e capabilities vêm de um único registro de dialetos.

## Consequências

Parser, CodeMirror, completion e futuras features não criam cópias independentes de regras PostgreSQL, MySQL ou SQL Server.
