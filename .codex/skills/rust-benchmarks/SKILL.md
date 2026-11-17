---
name: rust-benchmarks
description: Medir throughput e alocações em parsing e serialização Rust.
---

# rust-benchmarks

Use criterion ou ferramenta equivalente com payloads realistas. Compare cópia versus borrow, registre latência e evite otimizações que alterem semântica do protocolo.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
