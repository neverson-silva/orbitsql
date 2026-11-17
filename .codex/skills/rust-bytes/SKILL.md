---
name: rust-bytes
description: Usar Bytes e buffers compartilhados em parsing TDS.
---

# rust-bytes

Defina claramente quando usar Bytes, BytesMut e Vec. Reuse capacidade sem reter buffers gigantes e preserve a delimitação de pacote ao fazer split.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
