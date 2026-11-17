---
name: rust-serialization
description: Serializar parâmetros e resultados em formatos Rust estáveis.
---

# rust-serialization

Mapeie tipos Rust para tipos TDS com regras explícitas de nullability, precisão e escala. Rejeite conversões ambíguas em vez de truncar silenciosamente.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
