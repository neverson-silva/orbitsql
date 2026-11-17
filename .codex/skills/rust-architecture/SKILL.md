---
name: rust-architecture
description: Projetar crates Rust para um driver TDS sustentável, separando protocolo, transporte, sessão e API pública.
---

# rust-architecture

Defina módulos por responsabilidade: API, sessão, transporte e wire protocol. Mantenha tipos de domínio independentes de Tokio e exponha traits para permitir testes e transportes alternativos.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
