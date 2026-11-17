---
name: rust-traits-generics
description: Projetar traits e generics para abstrações de conexão e codec.
---

# rust-traits-generics

Mantenha traits pequenas e orientadas a capacidades. Use generics no núcleo quando eliminarem custo, mas objetos trait na API quando facilitarem extensibilidade.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
