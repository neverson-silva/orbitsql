---
name: compatibility-testing
description: Executar matriz de compatibilidade contra versões e modos MSSQL.
---

# compatibility-testing

Varie SQL auth, TLS, collations, IPv4/IPv6, instância, Azure SQL e versões suportadas. Separe falha de ambiente de regressão de protocolo.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
