---
name: tedious-streaming
description: Portar streaming de rows e controle de fluxo.
---

# tedious-streaming

Conecte o produtor TDS ao consumidor sem crescimento ilimitado de memória. Documente pause/resume, high-water mark, cancelamento e erro durante consumo.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
