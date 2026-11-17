---
name: port-planning
description: Planejar o port do Tedious para Rust com rastreabilidade.
---

# port-planning

Divida por camadas e marcos verticais. Para cada recurso, registre comportamento de origem, implementação-alvo, fixture, risco e critério de compatibilidade.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
