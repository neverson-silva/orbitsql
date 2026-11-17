---
name: rust-concurrency
description: Coordenar requisições, respostas e cancelamento concorrente.
---

# rust-concurrency

Imponha uma máquina de estados que impeça requests incompatíveis na mesma sessão. Use canais limitados, shutdown coordenado e garanta que erros acordem todos os aguardantes.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
