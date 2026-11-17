---
name: rust-async-tokio
description: Implementar I/O assíncrono de driver SQL Server com Tokio.
---

# rust-async-tokio

Prefira AsyncRead/AsyncWrite e backpressure explícito. Não bloqueie o runtime; delimite timeouts, cancelamento e shutdown em cada operação de rede.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
