# Auditoria de tokens das telas Stitch

Escopo auditado: as doze telas Stitch implementadas no frontend, seus componentes compartilhados, shell, editor e as trinta e duas unidades `.tsx` sob `src`.

## Auditoria 1: cores e tipografia

- Todas as classes com cores literais, `rgba`, paletas Tailwind fixas e valores tipográficos arbitrários foram removidas das unidades `.tsx`.
- As telas usam papéis semânticos como `surface-container`, `on-surface`, `primary`, `success`, `warning` e `error`.
- Tipografia usa os contratos `display`, `headline`, `body`, `code` e `label` definidos pelo tema.
- `font-sans` usa a fonte da UI e `font-mono` usa a fonte do editor, ambas substituíveis pelo store persistido.
- O CodeMirror recebe a mesma identidade de tema e as preferências de fonte do editor.

## Auditoria 2: espaçamento, densidade e raios

- Espaçamentos inteiros e fracionários usados pelas telas são resolvidos pelos namespaces de espaçamento do Tailwind 4 ligados a `--ds-space-*` e `--ds-spacing-unit`.
- `compact`, `normal` e `comfortable` recalculam a unidade de espaçamento antes do primeiro render.
- Raios usados pelas telas são `rounded-sm`, `rounded`, `rounded-md`, `rounded-lg`, `rounded-xl` e `rounded-full`, todos ligados às escalas `--ds-radius-*`.
- Raios sem escala explícita encontrados na revisão foram normalizados para `rounded-md`.
- O anel de telemetria usa `border-ring`, cuja largura deriva da densidade atual.
- Larguras, proporções de grid e posições percentuais do ERD permanecem como geometria estrutural da composição, não como valores de cor, tipografia, espaçamento ou raio.

## Auditoria 3: qualidade, acessibilidade e fronteiras

- Todos os botões diretos declaram `type` e os controles interativos compartilham foco visível com a cor primária do tema.
- Campos de formulário possuem label associado ou `aria-label` apropriado.
- Estilos inline restantes são exclusivamente dados visuais: percentuais de progresso, altura de gráficos e amostras de cores vindas do registro de temas.
- As doze telas usam dados locais de demonstração e não importam backend, IPC, credenciais ou cliente de banco.
- As referências Stitch foram baixadas para `/private/tmp/orbitsql-stitch-audit`, com doze imagens e doze arquivos HTML usados na conferência visual.

## Verificações executadas

- `pnpm check`
- `pnpm typecheck`
- `pnpm test`: 69 testes passando
- `pnpm build`
- `cargo check -p orbitsql-ui`
