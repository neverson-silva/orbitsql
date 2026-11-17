---
name: mssql-sql
description: Gerar e executar mensagens SQL Server sobre TDS.
---

# mssql-sql

Separe SQL textual de RPC e parâmetros. Preserve encoding, tipos declarados, ordem, result sets e regras de batch sem montar SQL por concatenação insegura.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
