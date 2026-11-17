---
name: typescript-interop
description: Implementar interop Node/TypeScript com bindings Rust.
---

# typescript-interop

Escolha N-API, napi-rs ou outra ponte com base em lifecycle e distribuição. Converta erros e eventos de forma lossless e teste shutdown, GC e backpressure.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
