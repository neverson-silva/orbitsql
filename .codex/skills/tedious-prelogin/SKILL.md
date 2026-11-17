---
name: tedious-prelogin
description: Implementar negociação PRELOGIN compatível com SQL Server.
---

# tedious-prelogin

Codifique tabela de opções, terminator, versões e encryption flag. Faça a decisão de TLS obedecer servidor, configuração do cliente e política de segurança.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
