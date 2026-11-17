# ADR-012: Semantic form control surfaces

## Contexto

Os primitives de entrada combinavam superfícies semânticas com degraus arbitrários da escala Material. Em temas escuros, `surface-container-low`, transparência e `--input` usado como background podiam transformar campos ativos em caixas quase pretas ou visualmente desabilitadas. Menus flutuantes também compartilhavam o mesmo papel de superfície dos campos. O shell ainda comunicava sua estrutura com saltos tonais grandes entre chrome, sidebar, workspace, editor e status bar.

## Decisão

O Theme Engine deriva papéis semânticos runtime para a hierarquia de superfícies e para controles incorporados:

- `app-chrome` é uma única superfície compartilhada por title bar, activity bar, sidebar, tab strip e status bar. `app-chrome-subtle` é reservado a estados locais dentro desse chrome. `app-content`/`app-content-subtle` representam conteúdo, editor, resultados, toolbars locais e headers de resultados. O chrome é derivado em OKLCH com lightness ligeiramente menor que o conteúdo em qualquer modo de tema.
- `app-shell`, `app-sidebar`, `app-workspace`, `app-editor` e `app-results` permanecem como aliases semânticos compatíveis; `surface-subtle`, `surface-raised`, `surface-divider` e `structural-divider` completam a hierarquia sem depender de steps Material arbitrários.
- `selected-surface` representa seleção e estados ativos sem transformar todo estado interativo em `primary`.

- `control-surface`, seus estados e bordas representam Input, Select, Textarea, NativeSelect, InputGroup, Combobox e controles equivalentes.
- `floating-surface` e `floating-border` representam SelectContent, ComboboxContent, Popover e DropdownMenu.
- `--input` continua sendo o token de borda compatível com shadcn; não representa background.
- Estados default, hover, focus-visible, popup-open, pressed, disabled, invalid e readonly consomem os papéis semânticos.
- Densidade e raio continuam vindo do Theme Engine; nenhuma palette existente recebe override por tema ou tela.

## Consequências

Fields mantêm uma diferença tonal pequena em relação ao modal, recebem borda legível e preservam hierarquia própria para popups. Novos primitives devem consumir `formControlClassName` ou os papéis `control-*`, sem escolher diretamente `surface-container-lowest` para representar um campo. Superfícies grandes devem preferir estrutura, divider e tipografia; somente elementos embedded e floating recebem deltas progressivamente maiores.

## Verificação

A playground `/form-controls` renderiza a matriz de estados e o Surface Debug lado a lado, permitindo alternar todos os temas registrados para inspeção visual do contrato compartilhado.
