---
name: interop-debugging
description: Depurar divergências entre Tedious, tedious-rs e SQL Server.
---

# interop-debugging

Compare captura de pacotes sanitizada, estados, tokens e eventos no mesmo cenário. Reduza para menor fixture reproduzível antes de corrigir o parser ou a API.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
