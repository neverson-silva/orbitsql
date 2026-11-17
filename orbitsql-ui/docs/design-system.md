# OrbitSQL Design System

## Contrato

`src/design-system` concentra a infraestrutura visual da aplicação, enquanto os componentes compartilhados em `src/components/ui` consomem os mesmos papéis semânticos.

O tema inicial é `precision-glass-studio`, derivado de `DESIGN.md`. O registry também inclui os vinte presets que existiam em `never-sql-navigator-ui`, em `src/themes/builtin`: Eclipse, Horizon, High Contrast, Dark+ (VS Code), One Dark Pro, Dracula, Monokai, Nord, GitHub Dark, GitHub Light, Solarized Dark, Solarized Light, Material Darker, Material Ocean, Darcula, Rider Dark, Ayu Dark, Ayu Mirage, Ayu Light e Night Owl.

Os presets legados mantêm os valores de cores e syntax do arquivo de origem:

`/Users/neverson/myspace/legacy_space/OLD/APP/never-sql/never-sql-navigator-ui/src/themes/builtin`

Eles são adaptados aos papéis semânticos do OrbitSQL para que a mesma paleta possa alimentar o shell, componentes futuros, Tailwind e CodeMirror.

### Superfícies

`ThemeColors` é a paleta de materiais do tema; não é o mapa final da
interface. `resolveSurfaceRoles` traduz essa paleta para um conjunto pequeno
de planos estruturais:

- `surfaceDim` alimenta o chrome profundo;
- `surfaceContainerLow` alimenta a navegação lateral;
- `background` é o plano de trabalho compartilhado pelo editor e resultados;
- `surfaceContainer` alimenta toolbars e cabeçalhos de comando;
- temas claros usam `surfaceContainerLowest` para raised e floating;
- temas escuros usam `surfaceContainerHigh` e `surfaceContainerHighest` para
  raised e floating;
- controls usam um container apropriado ao modo e seus estados são derivados
  desse papel.

Esses nomes descrevem materiais disponíveis, enquanto `app-sidebar`,
`app-toolbar`, `app-editor` e `surface-raised` descrevem papéis de uso.
Editor e resultados podem resolver para a mesma cor sem perder separação
estrutural: splitter, dividers, agrupamento e tipografia continuam sendo os
outros elementos da gramática visual. A matriz `/form-controls` mostra todos
os temas e os valores `L*` dos planos para inspeção.

O mapper não usa offsets fixos de OKLCH para construir as superfícies
estruturais. `oklch()` e `color-mix()` continuam reservados a estados locais,
controles e diálogos, onde representam variação contextual. `--input` segue
sendo a borda de controle compatível com shadcn.

## Aplicação

`themeRegistry` é o registro extensível de temas. Ele suporta registro, remoção de temas não embutidos, aplicação, preview, cancelamento de preview, validação, exportação e importação versionada.

`applyDesignSystem` resolve o tema persistido, aplica seus tokens no `document.documentElement` e aplica as preferências independentes de tema:

- `themeId` seleciona o tema;
- `uiFontFamily` e `uiFontSize` controlam a interface;
- `editorFontFamily` e `editorFontSize` controlam o CodeMirror;
- `density` usa `compact`, `normal` ou `comfortable`;
- `radius` controla a escala global de raios.

O store Zustand mantém essas preferências junto de `sidebarOpen`. O adapter já existente de `@tauri-apps/plugin-store` persiste tudo em `settings.json`, sem armazenar credenciais.

A persistência usa hidratação explícita com `skipHydration`. `src/main.tsx` aguarda `hydrateSettings`, aplica o snapshot resolvido no documento e só então monta o React. Com isso, o primeiro render já recebe o tema salvo; quando não existe snapshot, o estado inicial usa `DEFAULT_DESIGN_SETTINGS`. O shell não reaplica os tokens em cada render: setters de design aplicam somente quando uma preferência muda.

## Tailwind 4

O projeto já usa `@import "tailwindcss"` e `@tailwindcss/vite`. `src/styles.css` declara `@theme inline` e conecta os namespaces do Tailwind às variáveis `--ds-*` da engine. Portanto, utilitários de cor, fonte, espaçamento, radius, tipografia e syntax podem ser usados sem um `tailwind.config`.

As classes antigas baseadas em `white` e `gray` também apontam para papéis semânticos, mantendo a aplicação preparada para a troca de tema sem exigir migração imediata das telas.

## CodeMirror

`createCodeMirrorTheme` traduz as cores semânticas e syntax do tema para uma extensão CodeMirror. `themeCompartment` permite atualizar tema e preferências de editor sem recriar o documento ou perder o estado da edição.
