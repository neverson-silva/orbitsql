---
name: tds-collation
description: Interpretar e transportar metadados de collation.
---

# tds-collation

Preserve LCID, flags e sort ID sem inventar conversões. Defina comportamento para collations desconhecidas e mantenha bytes originais quando possível.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
