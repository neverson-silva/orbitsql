---
name: mssql-transactions
description: Analisar semântica de transações e comandos MSSQL.
---

# mssql-transactions

Cubra autocommit, savepoints, XACT_ABORT, deadlocks e desconexão. Não reporte commit antes do DONE correspondente e faça rollback seguro quando possível.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
