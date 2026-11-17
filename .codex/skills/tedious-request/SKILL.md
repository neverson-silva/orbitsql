---
name: tedious-request
description: Portar Request, parâmetros e streaming de resultados.
---

# tedious-request

Preserve ordem de row, column metadata, done, return status e errors. Trate cancelamento e callbacks sem emitir completion duplicado.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
