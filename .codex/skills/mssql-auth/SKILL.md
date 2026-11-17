---
name: mssql-auth
description: Implementar autenticação SQL, Windows e modernos métodos MSSQL.
---

# mssql-auth

Modele credenciais e tokens sem expô-los. Negocie o método suportado e produza erros acionáveis para credencial inválida, TLS obrigatório e token expirado.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
