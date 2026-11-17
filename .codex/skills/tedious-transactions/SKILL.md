---
name: tedious-transactions
description: Portar begin, commit, rollback e abort do Tedious.
---

# tedious-transactions

Defina o estado transacional e o que acontece após erro ou desconexão. Preserve isolamento, callbacks e prevenção de requests concorrentes incompatíveis.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
