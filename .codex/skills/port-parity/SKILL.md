---
name: port-parity
description: Verificar paridade funcional entre Tedious e tedious-rs.
---

# port-parity

Compare resultados, eventos, erros, timing observável e defaults. Trate divergências como decisões documentadas ou defeitos, nunca como equivalência presumida.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
