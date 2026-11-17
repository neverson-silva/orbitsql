---
name: tds-packets
description: Especializar framing, headers e tipos de pacotes TDS.
---

# tds-packets

Valide comprimento declarado, SPID, packet ID e EOM. Reagrupe fragmentos e rejeite sequência impossível antes de passar dados ao tokenizer.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
