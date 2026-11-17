---
name: rust-networking
description: Implementar transporte TCP robusto para SQL Server.
---

# rust-networking

Cubra resolução de endereço, conexão, half-close, leitura parcial e escrita parcial. Faça o transporte obedecer timeout e cancelamento sem perder bytes.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
