---
name: rust-ffi
description: Projetar fronteiras FFI para integrar o driver Rust com TypeScript/Node.
---

# rust-ffi

Defina ABI C estável, ownership explícito e códigos de erro. Não atravesse panic, referências Rust ou tipos não repr(C) pela fronteira.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
