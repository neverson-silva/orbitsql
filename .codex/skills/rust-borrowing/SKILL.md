---
name: rust-borrowing
description: Resolver ownership e borrowing em buffers, tokens e resultados SQL.
---

# rust-borrowing

Escolha ownership quando os dados ultrapassarem o pacote de origem. Use slices somente enquanto o buffer estiver vivo e evite referências atravessando await points.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
