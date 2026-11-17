---
name: tds-tls
description: Diagnosticar TLS dentro do handshake TDS.
---

# tds-tls

Diferencie encryption off, on, not supported e required. Registre apenas metadados seguros e teste certificados, SNI, renegociação proibida e falhas de versão.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
