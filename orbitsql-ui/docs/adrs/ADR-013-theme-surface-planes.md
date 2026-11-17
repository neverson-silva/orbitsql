# ADR-013: Planos de superfície derivados da paleta do tema

## Contexto

O mapper anterior gerava chrome, sidebar, toolbar, editor e resultados quase
inteiramente a partir de `colors.background` com pequenos deltas fixos. Isso
criava muitos nomes semânticos para poucos planos perceptuais e fazia a
hierarquia depender de bordas. Também tornava a mesma fórmula frágil entre os
temas claros, escuros e cromáticos do registry.

## Decisão

`ThemeColors` permanece a camada de materiais. `resolveSurfaceRoles` é a
camada que atribui esses materiais aos papéis estruturais da aplicação.

- `surfaceDim` é o chrome profundo.
- `surfaceContainerLow` é a superfície estrutural da navegação.
- `background` é o plano de trabalho de editor e resultados.
- `surfaceContainer` é o plano de command chrome compartilhado por toolbars.
- No modo claro, `surfaceContainerLowest` representa superfícies raised e
  floating; sombra e borda preservam a distinção de overlay.
- No modo escuro, `surfaceContainerHigh` e `surfaceContainerHighest`
  representam superfícies raised e floating.
- `control-surface` usa um container do mesmo sistema e hover, focus, active,
  disabled e readonly continuam sendo derivados de seu papel base.

Os papéis podem compartilhar um material. Em particular, editor e resultados
continuam no mesmo work plane. Estados locais podem usar `oklch()` e
`color-mix()`; a estrutura não usa offsets arbitrários de uma única cor.

O mapper não altera primary, secondary, warning, error, success ou as cores
de objetos de banco. A solução de dialogs existente permanece inalterada,
assim como o contrato de `--input` como borda de controle.

## Consequências

Novos temas precisam declarar uma paleta de containers coerente com o modo do
tema. Um tema pode preservar sua identidade cromática e ainda receber uma
hierarquia previsível de planos. A matriz de superfícies e o diagnóstico puro
em `src/design-system/surfaces.ts` tornam a relação entre paleta, papéis,
OKLCH `L*` e luminância relativa verificável sem console spam de produção.

## Verificação

`src/design-system/surfaces.test.ts` verifica todos os temas embutidos, a
origem dos papéis na paleta e a disponibilidade de métricas numéricas. A
playground `/form-controls` permite inspeção visual de todos os papéis e
temas, incluindo a prévia em escala de cinza.
