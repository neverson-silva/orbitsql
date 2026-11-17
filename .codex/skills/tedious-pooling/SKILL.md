---
name: tedious-pooling
description: Projetar pool compatível com o uso de conexões Tedious.
---

# tedious-pooling

Defina aquisição, idle timeout, health check e descarte de conexões inválidas. Nunca devolva ao pool uma sessão com request, transação ou resultados ativos.

## Critérios

- Mantenha compatibilidade observável e documente qualquer divergência.
- Valide com testes determinísticos, incluindo entradas inválidas e limites.
- Preserve segurança, cancelamento e diagnósticos úteis.
