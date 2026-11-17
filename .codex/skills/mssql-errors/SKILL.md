---
name: mssql-errors
description: Interpretar ERROR, INFO e mensagens de falha do SQL Server.
---

# mssql-errors

Extraia número, estado, classe, servidor, procedure e linha. Preserve múltiplas mensagens e diferencie severidade informativa de falha terminal.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
