---
name: tedious-tokenizer
description: Portar o tokenizer do Tedious para Rust.
---

# tedious-tokenizer

Implemente tokens com limites rigorosos e estados incrementais. Compare cada token com fixtures do projeto original e mantenha offsets e mensagens úteis para diagnóstico.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
