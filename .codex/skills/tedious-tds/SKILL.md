---
name: tedious-tds
description: Implementar o fluxo TDS usado pelo Tedious.
---

# tedious-tds

Trate header, packet type, status, comprimento e sequência. Separe framing de tokens e nunca presuma que uma leitura TCP corresponde a um pacote TDS.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
