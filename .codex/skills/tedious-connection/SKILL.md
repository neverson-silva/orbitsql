---
name: tedious-connection
description: Portar estados e lifecycle de Connection do Tedious.
---

# tedious-connection

Modele connecting, logged-in, streaming, closing e closed explicitamente. Garanta que callbacks/eventos ocorram uma única vez e que fechamento libere requests pendentes.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
