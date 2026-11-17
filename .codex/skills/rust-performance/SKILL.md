---
name: rust-performance
description: Otimizar um cliente Rust sem sacrificar compatibilidade TDS.
---

# rust-performance

Meça antes de alterar. Priorize menos cópias e alocações no caminho quente, mas preserve limites, ordenação, cancelamento e diagnósticos observáveis.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
