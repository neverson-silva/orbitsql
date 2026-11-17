---
name: rust-tls
description: Integrar TLS em conexões TDS sem quebrar o fluxo de prelogin.
---

# rust-tls

Modele TLS como camada sobre o transporte após negociação. Valide hostname, cadeia de certificados e políticas configuráveis; nunca desative verificação por padrão.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
