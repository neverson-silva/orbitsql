---
name: rust-lifetimes
description: Projetar lifetimes para views de pacotes TDS e valores decodificados.
---

# rust-lifetimes

Documente quem possui cada buffer. Faça views emprestadas explícitas e converta para tipos owned na fronteira pública ou quando houver armazenamento assíncrono.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
