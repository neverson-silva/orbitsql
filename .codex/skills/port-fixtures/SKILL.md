---
name: port-fixtures
description: Criar fixtures compartilhadas para portabilidade TypeScript-Rust.
---

# port-fixtures

Capture bytes, configuração mínima e resultado esperado sem segredos. Inclua casos de sucesso, fragmentação, unicode, nulls, erros e versões de servidor.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
