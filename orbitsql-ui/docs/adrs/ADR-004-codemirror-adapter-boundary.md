# ADR-004 CodeMirror adapter boundary

## Decisão

CodeMirror fornece `EditorState`, árvore Lezer e cancelamento; um adapter converte isso em requests do core e mapeia o resultado para `CompletionResult`.

## Consequências

O core não importa APIs CodeMirror e pode ser executado por parser de testes ou futuro worker.
