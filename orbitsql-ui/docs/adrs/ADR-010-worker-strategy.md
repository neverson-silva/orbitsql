# ADR-010 Worker strategy

## Decisão

O motor puro continua sem dependências de UI e mantém um fallback síncrono para ambientes sem Worker. No desktop, completions interativas rodam em um único module worker compartilhado por todas as abas, com um snapshot de catálogo por versão e um documento replicado por deltas de CodeMirror.

Cada request carrega o identificador e a versão do documento, recebe no máximo 100 itens e usa latest-wins por documento. O cancelamento usa `SharedArrayBuffer` quando disponível e retorna ao protocolo de mensagem quando não estiver. O runtime libera o snapshot no worker quando a última sessão deixa de referenciá-lo.

## Consequências

O primeiro completion de um catálogo paga a construção do índice no worker e cada resposta paga apenas a serialização de uma lista limitada. O worker mantém uma cópia estruturada do snapshot, mas ela não é duplicada por editor. Mudanças de texto não enviam o documento inteiro nem bloqueiam o event loop da UI.

A lane interativa é deliberadamente separada de qualquer trabalho futuro de manutenção ou introspecção. Pools de background, catálogo binário em memória compartilhada e fan-out de candidatos permanecem extensões condicionadas a medições do engine real no WebView.
