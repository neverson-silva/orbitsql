---
name: typescript-architecture
description: Projetar a API TypeScript equivalente ao Tedious sobre um núcleo Rust.
---

# typescript-architecture

Preserve contratos públicos, eventos e opções observáveis. Isole bindings, adaptadores e tipos; não deixe detalhes do wire protocol vazarem para usuários comuns.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
