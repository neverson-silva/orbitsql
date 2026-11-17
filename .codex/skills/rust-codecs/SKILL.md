---
name: rust-codecs
description: Implementar codecs binários eficientes e verificáveis em Rust.
---

# rust-codecs

Centralize leitura e escrita endian-aware com checagem de limites. Retorne erro em overflow, comprimento inválido ou enum desconhecido, sem panic em bytes externos.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
