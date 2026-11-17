# OrbitSQL

OrbitSQL é uma aplicação desktop de consulta e exploração de bancos de dados, construída como alternativa focada a ferramentas como DBeaver. O objetivo é combinar conexões reais, navegação de catálogo e um editor SQL com assistência contextual em uma interface leve.

**Estado atual: alpha.** O produto já conecta a alguns bancos e executa consultas, mas ainda não oferece a cobertura de drivers e ferramentas de administração de um cliente maduro como o DBeaver. Consulte [O que está pronto e o que falta](#o-que-está-pronto-e-o-que-falta) antes de usar em fluxos de trabalho importantes.

## O que está pronto e o que falta

### Implementado

- Aplicação desktop para macOS, Windows e Linux, usando Tauri 2.
- Conexões reais com PostgreSQL e Microsoft SQL Server. SQL Server oferece SQL Authentication e Windows Authentication via NTLM.
- Teste, salvamento, edição, reconexão e remoção de perfis de conexão.
- Navegação do catálogo do banco com schemas, tabelas, views, colunas, tipos e metadados disponíveis por driver.
- Editor SQL CodeMirror com abas de consulta, highlighting e completions contextuais baseadas no catálogo carregado.
- Execução de SQL, resultados em grade, sessões de resultado paginadas e cancelamento de consulta.
- Inspeção de metadados de tabelas e monitor de sessões para SQL Server.
- Senhas separadas dos perfis persistidos, com armazenamento no serviço de credenciais do sistema operacional. Há também um cofre local de segredos.
- Conexões por SSH e proxy HTTP CONNECT/SOCKS5 estão previstas no modelo de transporte da aplicação.

### Ainda não disponível ou incompleto

- MySQL e MariaDB aparecem no seletor e há código de introspecção, mas seus drivers não estão habilitados para conexão na aplicação.
- SQLite, Oracle e Redis não têm conexão implementada. Seus nomes/dialetos no editor não significam suporte a driver.
- As telas de dados e diagrama de uma tabela ainda não carregam linhas nem um ERD. A tela de propriedades apresenta metadados disponíveis; use uma consulta SQL para ler dados.
- Edição de dados de tabela por grade ainda não está implementada. Alterações podem ser feitas por SQL, conforme as permissões do banco.
- O produto não pretende, nesta fase, cobrir toda a administração, ferramentas visuais e variedade de drivers do DBeaver.
- O driver `tedious-rs` é o port Rust do Tedious para SQL Server. Ele está integrado ao caminho SQL Server do OrbitSQL, mas seu escopo de paridade é acompanhado separadamente e não equivale a declarar o port inteiro concluído.

## Decisões de produto e arquitetura

- **Desktop nativo com web UI:** Tauri 2 hospeda React/TypeScript e usa Rust para acesso ao sistema e aos bancos.
- **Rust é responsável pelos fatos do banco:** conexão, introspecção e normalização de catálogo ficam no backend. A interface recebe um snapshot comum entre os drivers.
- **TypeScript é responsável pela inteligência SQL local:** parser, escopos, aliases, CTEs, inferência contextual e ranking de completions ficam no frontend. A completion trabalha sobre um catálogo imutável, sem consultar o banco a cada tecla.
- **Editor desacoplado da camada visual:** CodeMirror é integrado por um adaptador fino; o núcleo de inteligência SQL pode ser exercitado sem React, Tauri ou conexão ativa.
- **Segredos fora das preferências comuns:** perfis persistidos guardam metadados de conexão; senhas são encaminhadas ao backend quando necessário e armazenadas no serviço de credenciais do sistema ou no cofre local.
- **Backend organizado por adapters:** os comandos Tauri encaminham operações para portas e adapters de banco, mantendo o formato canônico do catálogo independente do fornecedor.

As decisões estão detalhadas em [`orbitsql-ui/docs/adrs/`](orbitsql-ui/docs/adrs/) e as capacidades de conexão em [`orbitsql-ui/src-tauri/specs/`](orbitsql-ui/src-tauri/specs/).

## Requisitos para desenvolvimento local

- Git.
- Rust estável via `rustup` (o workspace usa edition 2024).
- Node.js LTS e Bun. O repositório mantém `bun.lock` e o Tauri chama `bun run` nos passos de desenvolvimento e build.
- Dependências nativas do Tauri para o seu sistema operacional:
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`).
  - **Windows:** Microsoft C++ Build Tools com “Desktop development with C++” e WebView2 Runtime.
  - **Linux Debian/Ubuntu:** `libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev` e `librsvg2-dev`.
- Um servidor PostgreSQL ou SQL Server acessível só é necessário para testar conexões e consultas pelo produto. Não é necessário para abrir a aplicação.
- Docker é opcional e necessário apenas para o fluxo automatizado de introspecção contra PostgreSQL, MySQL e MariaDB.

Veja os [pré-requisitos oficiais do Tauri 2](https://v2.tauri.app/start/prerequisites/) para instruções específicas de distribuição Linux e configuração das ferramentas nativas.

## Rodar localmente

No terminal, a partir da raiz do repositório:

```sh
cd orbitsql-ui
bun install
bun run tauri dev
```

O comando inicia o Vite e abre a aplicação desktop pelo Tauri. Para usar um banco, crie uma conexão pela interface e informe o endereço e as credenciais do seu próprio ambiente. Senhas não devem ser adicionadas a arquivos `.env`, ao código ou a fixtures.

Para iniciar somente o servidor web do Vite:

```sh
bun run dev
```

Esse modo serve para trabalhar na interface; comandos nativos Tauri e conexões reais dependem do processo desktop.

## Build

Build do frontend, verificação de tipos e geração dos bundles/instaladores Tauri:

```sh
cd orbitsql-ui
bun install
bun run tauri build
```

Os artefatos são gerados sob `target/release/bundle/` na raiz do workspace Rust. Os formatos disponíveis dependem do sistema operacional e das ferramentas de empacotamento instaladas.

Para gerar apenas os arquivos web do frontend:

```sh
bun run build
```

## Verificações de desenvolvimento

Execute em `orbitsql-ui/`:

```sh
bun run check
bun run typecheck
bun run test:coverage
bun run test:rust
```

Os testes de adapters reais para PostgreSQL, MySQL e MariaDB usam Docker e podem ser executados com:

```sh
bun run test:rust:providers
```

Esse fluxo é independente da lista de bancos que a aplicação habilita para conexão. As integrações que dependem de servidores externos têm perfis de teste próprios e não devem receber credenciais de produção.

## Estrutura do repositório

```text
orbitsql-ui/   Aplicação desktop, interface, backend Tauri e documentação do produto
tedious-rs/    Port Rust do driver Tedious para SQL Server
tedious-node/  Implementação Node de referência para o port; não é a aplicação OrbitSQL
specs/         Evidências e contratos do port tedious-node → Rust
```

O workspace Cargo está na raiz. Os comandos de frontend e Tauri devem ser executados de `orbitsql-ui/`.
