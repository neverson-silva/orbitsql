# AGENTS.md — OrbitSQL UI

## Escopo

Este diretório contém a aplicação desktop do OrbitSQL. O backend nativo é o crate Tauri em `src-tauri`; o frontend é React com TypeScript e Vite.

`tedious-node` não faz parte deste projeto e não deve ser alterado. O crate `tedious-rs` é o primeiro membro do workspace de backend do OrbitSQL e a UI permanece desacoplada dele até existir uma integração explícita.

## Stack obrigatória

- Tauri 2 com Rust em `src-tauri`.
- React com TypeScript 7.
- Zustand como fonte principal de estado da UI.
- `@tauri-apps/plugin-store` para persistência de settings em arquivo.
- TanStack Router com rotas file-based em `src/routes`.
- Tailwind CSS 4 pelo plugin do Vite.
- Biome como formatter e linter único.

ESLint e Prettier não devem ser adicionados. Se uma dependência ou template trouxer qualquer configuração dessas ferramentas, remover antes de concluir a alteração.

## Organização simples

Manter a organização por responsabilidade próxima do uso:

- `src/routes/` contém páginas e configuração de rotas.
- `src/components/` contém componentes reutilizáveis de UI.
- `src/stores/` contém stores Zustand e adapters de persistência.
- `src/main.tsx` cria o router e monta o React.
- `src-tauri/src/` contém somente a inicialização Tauri e comandos nativos necessários.

Não criar camadas de domínio, clean architecture, services, repositories ou módulos genéricos sem uma necessidade concreta da UI.

## Inteligência SQL

O motor de completions fica em `src/sql-intelligence` e é uma biblioteca pura de TypeScript. Ele recebe o texto, a posição, a versão do documento, um `CatalogIndex` imutável e um `AbortSignal` opcional; não importa React, Zustand, Tauri, filesystem, rede ou banco de dados.

O catálogo canônico é um tipo de domínio em `src-tauri/src/backend/domain/catalog.rs` e convertido para os tipos normalizados do frontend. O frontend mantém somente o snapshot em Zustand e nunca consulta banco ou disco durante uma completion.

Os únicos providers de catálogo desta fase são PostgreSQL, MySQL e MariaDB. PostgreSQL usa `sqlx-postgres`; MySQL e MariaDB usam `sqlx-mysql`. Eles normalizam schemas, tabelas, views, colunas, defaults, comentários, chaves, constraints, índices, funções, procedures, sequências e tipos para `CatalogSnapshot`. Senhas entram apenas na requisição explícita de refresh, não são persistidas no store de UI e não entram no snapshot. Não há provider SQL Server, NTLM ou conexão de banco no hot path de completion.

A comunicação nativa segue arquitetura hexagonal. `src-tauri/src/backend/ports` define ports pequenos para providers e factories; `src-tauri/src/backend/application` contém o caso de uso de refresh; `src-tauri/src/backend/adapters/drivers` contém os adapters PostgreSQL, MySQL e MariaDB; `src-tauri/src/backend/adapters/factory.rs` seleciona o adapter; `src-tauri/src/backend/commands.rs` é somente o adapter Tauri. O domínio não conhece SQLx, Tauri ou o runtime.

Os testes externos dos providers ficam em `tedious-rs/tests/integration/orbitsql_ui_introspection.rs` e são ignorados por padrão porque exigem bancos reais. Executá-los com `cargo test -p orbitsql-ui --test orbitsql_ui_introspection -- --ignored` e os perfis `ORBITSQL_TEST_POSTGRES_*`, `ORBITSQL_TEST_MYSQL_*` e `ORBITSQL_TEST_MARIADB_*` configurados em runtime.

O caminho reproduzível para desenvolvimento é `pnpm test:rust:providers`. Ele sobe `docker-compose.yml` com PostgreSQL, MySQL 8.4 e MariaDB, injeta fixtures de catálogo, executa os três providers, verifica cobertura estável de linhas/funções e cobertura de branches com o toolchain nightly, e remove os containers e volumes ao finalizar. O nightly precisa estar instalado com `llvm-tools-preview`.

A separação obrigatória é:

- `src-tauri/src/backend/domain/catalog.rs` representa fatos normalizados do banco.
- `src-tauri/src/backend/ports` define as fronteiras de comunicação do backend.
- `src-tauri/src/backend/application` orquestra casos de uso sem conhecer drivers.
- `src-tauri/src/backend/adapters` implementa drivers e seleção por factory.
- `src/sql-intelligence/catalog`, `dialect`, `parser`, `semantic` e `completion` resolvem SQL, escopos, tipos, aliases, CTEs, tabelas derivadas e relações.
- `src/editor/codemirror` adapta o motor puro para CodeMirror e deve permanecer fino.
- `src/stores` guarda estado da UI; stores não devem conter parsing ou ranking de SQL.

Decisões de arquitetura ficam em `docs/adrs/`. Não adicionar IA, LLM, embeddings ou IPC por tecla neste estágio.

O núcleo deve manter cobertura Vitest global mínima de 95% para statements, branches, functions e lines:

```text
pnpm test:coverage
```

## Estado e persistência

Componentes devem consumir estado por hooks Zustand. Não criar Context global para substituir stores Zustand.

Settings persistentes devem usar o adapter existente baseado em `@tauri-apps/plugin-store`. O arquivo de persistência é `settings.json`; alterações devem chamar `save()` explicitamente.

Não armazenar senhas, tokens, connection strings completas ou dados sensíveis em stores de UI.

## Rotas

Adicionar uma tela criando o arquivo correspondente em `src/routes`. O `routeTree.gen.ts` é gerado pelo plugin do TanStack Router e não deve ser editado manualmente.

Registrar a instância do router por module declaration para preservar inferência de tipos nas navegações.

## Código

O frontend deve ser orientado a hooks e manter componentes pequenos. Evitar lógica de negócio dentro de arquivos de configuração.

Não adicionar comentários em código Rust, TypeScript, TSX, JavaScript, CSS ou configuração executável. Decisões e contratos devem ficar neste arquivo ou em documentação Markdown.

## Validação

Executar a partir de `orbitsql-ui`:

```text
pnpm check
pnpm typecheck
pnpm build
cargo check -p orbitsql-ui
```

Executar a partir da raiz quando a alteração envolver o workspace Rust:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
```

Para alterações no editor ou no motor SQL, executar também `pnpm test:coverage`.
