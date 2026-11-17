---
name: rust-security
description: Revisar segurança de autenticação e transporte do cliente Rust.
---

# rust-security

Proteja segredos, minimize logs e valide entradas de rede. Audite downgrade TLS, nonce, buffers e erros que possam vazar credenciais ou consultas.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
