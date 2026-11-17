---
name: tds-encoding
description: Tratar codificação de texto no protocolo TDS.
---

# tds-encoding

Diferencie UTF-16LE, UTF-8 negociado, bytes binários e conversões de collation. Teste caracteres suplementares, strings vazias, terminadores e comprimentos em bytes.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
