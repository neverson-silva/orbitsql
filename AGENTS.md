# AGENTS.md — migração fiel do tedious-node para Rust

É TERMINANTEMENTE PROIBIDO USAR A VARIAVEL DE AMBIENTE TEDIOUS_RS_SQL_AUTH_PASSWORD

AS CREDENCIAIS PARA SQLAUTH COM USER E PASSWORD ESTAO NESTE ARQUIVO

## Escopo e objetivo

Este repositório contém o port de `tedious-node` (driver Node.js para SQL Server) para Rust, no diretório `tedious-rs`. O objetivo principal é obter uma implementação estável e funcional que preserve o comportamento do driver de origem com a maior fidelidade possível.

A prioridade do projeto é, nesta ordem:

1. Compatibilidade comportamental com o `tedious-node`.
2. Correção do protocolo TDS, transporte, autenticação e integração com SQL Server.
3. Testabilidade, rastreabilidade do port e estabilidade.
4. Idiomatismo, abstrações e otimizações específicas de Rust somente quando não reduzirem a paridade.

Este é um port, não uma reimplementação conceitual. Não substituir estruturas ou fluxos por uma arquitetura “mais Rust” sem evidência de que a mudança é necessária para compilar, funcionar, manter segurança ou respeitar as garantias de Rust.

## Proibição de comportamento inventado

O único objetivo de produção é portar o comportamento existente do `tedious-node` método a método, linha a linha e branch a branch. Não adicionar funcionalidades, políticas, validações, defaults, normalizações, eventos, conversões ou mudanças de API que não existam na referência.

Cada alteração de produção deve apontar para o arquivo, método e branch correspondente no `tedious-node`, ou para a adaptação mínima exigida por ownership, tipos, segurança de memória, ausência de uma API equivalente ou compilação Rust. Testes podem ser mais abrangentes para provar o comportamento, mas não autorizam alterar o comportamento observado.

Uma correção de segurança só pode mudar o resultado quando a referência produziria comportamento indefinido, panic por entrada externa ou uma operação impossível de representar com segurança em Rust. Nesse caso, preservar categoria, mensagem, estado e ordem observáveis tanto quanto possível e registrar a diferença em `specs/`.

Não criar abstrações, helpers, facades, APIs, codecs ou conveniências “melhores” sem um símbolo correspondente na referência. Não transformar uma divergência em equivalência apenas porque os testes locais passam. Um teste fake, fixture ou teste unitário prova somente o contrato que cobre; não prova paridade de métodos não exercitados nem integração real ausente.

## Regra de paridade linha a linha

O código de `tedious-node` deve ser tratado como a especificação primária. A migração deve ocorrer linha a linha, função por função e módulo por módulo:

- Preserve a mesma estrutura de diretórios e nomes de arquivos do projeto Node sempre que o sistema de arquivos permitir.
- Preserve nomes de classes, funções, métodos, propriedades, eventos, constantes, estados e tabelas de decisão.
- Em Rust, represente classes com `struct` + `impl`, herança com composição explícita e interfaces com `trait`, mantendo os nomes e responsabilidades originais.
- Preserve assinaturas conceituais, ordem de argumentos, valores padrão, efeitos colaterais, ordem de eventos e comportamento de erro.
- Mantenha nomes originais mesmo que não sejam idiomáticos em Rust; só adapte a grafia quando o compilador exigir ou quando a convenção for necessária para uma API pública Rust, documentando o mapeamento.
- Preserve estados internos, transições, callbacks, streams, listeners, códigos de erro e mensagens sempre que possível.
- Não remova uma função, classe, campo, estado ou caminho de erro apenas por parecer não utilizado.
- Não introduza uma abstração nova para “simplificar” código antes de existir uma implementação equivalente e testada.
- Não altere a ordem de parsing, serialização, leitura, escrita, emissão de eventos ou conclusão de requests.
- Não esconda divergências atrás de uma API diferente. Toda diferença deve ser registrada com a referência ao arquivo/linha do `tedious-node`, motivo e teste que a cobre.

### Dependências do Node e crates Rust

O comportamento do `tedious-node` pode estar dividido entre o código do pacote e as dependências instaladas em `node_modules`. Portanto, a análise da referência não pode parar nos arquivos do pacote principal:

- Identifique imports, `require`, exports, wrappers e chamadas delegadas para dependências externas.
- Quando uma função ou decisão depender de uma biblioteca em `node_modules`, leia o código efetivamente instalado, a versão resolvida e os pontos de entrada usados.
- Considere também subdependências quando elas influenciarem parsing, framing TDS, TLS, criptografia, NTLM, encoding, streams, pooling, DNS ou conversão de tipos.
- Registre a origem de cada comportamento delegado: arquivo do Tedious, pacote externo, versão e função relevante.
- Não reimplemente no Rust uma capacidade já fornecida de forma confiável por um crate mantido e compatível com o requisito.
- Antes de adotar um crate Rust, verifique licença, manutenção, compatibilidade de plataforma, segurança, API, limitações de protocolo e comportamento de erro.
- Prefira crates maduros para primitivas complexas, como runtime assíncrono, buffers, TLS, criptografia, NTLM, codecs, encoding e bindings Node, desde que a camada de compatibilidade preserve o comportamento do Node.
- O crate escolhido é um detalhe de implementação: não substitua a API, os nomes, os eventos, os defaults ou a semântica do `tedious-node` pela API do crate.
- Envolva dependências externas em adaptadores pequenos e testáveis. Preserve os dados intermediários, a ordem das operações e os erros observáveis.
- Se não houver crate adequado, implemente apenas a menor camada ausente e documente por que a dependência existente não atende.
- Não adicione dependências por preferência ou conveniência sem confirmar que elas reduzem risco real do port.

O resultado esperado é uma tradução fiel apoiada por bibliotecas Rust confiáveis, não uma cópia literal de toda capacidade que já exista no ecossistema. A decisão “reusar crate” versus “portar lógica” deve ser tomada por comportamento verificável e registrada com testes de equivalência.

Quando Rust exigir uma adaptação, use a menor transformação possível:

1. Identifique o trecho equivalente no `tedious-node`.
2. Traduza a lógica mantendo sequência e condições.
3. Preserve os dados intermediários e o estado observável.
4. Adicione apenas as verificações exigidas por ownership, tipos, limites, concorrência ou segurança.
5. Crie um teste que demonstre a equivalência.

Uma diferença só é aceitável quando a lógica original não pode ser reproduzida em Rust, quando seria insegura/indefinida, quando depende de uma API inexistente no sistema-alvo ou quando o compilador exige uma forma equivalente. “Mais elegante”, “mais idiomático” ou “mais performático” não são justificativas suficientes isoladamente.

## Estrutura do port

O diretório de trabalho do driver é `tedious-rs`. A estrutura de módulos deve espelhar a estrutura do `tedious-node` tanto quanto possível. O mapeamento deve ser explícito, por exemplo:

- arquivo/módulo Node → arquivo/módulo Rust equivalente;
- classe Node → `struct` e bloco `impl` Rust;
- método → método Rust com o mesmo papel e nome mapeado;
- evento/listener → enum de eventos e mecanismo de emissão equivalente;
- callback/Promise → resultado assíncrono equivalente, sem mudar a ordem de observação;
- `Buffer` → `Bytes`, `BytesMut` ou `Vec<u8>` somente após avaliar ownership e cópias;
- exceção → enum de erro preservando categoria, código, mensagem e contexto;
- `EventEmitter` → dispatcher/canal que preserve inscrição, emissão e encerramento;
- stream → produtor/consumidor com backpressure, pause/resume e cancelamento equivalentes.

Antes de portar um módulo, registre o caminho do arquivo de origem, o caminho de destino, as funções/classes cobertas e o status: não iniciado, em andamento, equivalente, divergente ou bloqueado.

## Protocolo, transporte e autenticação

O port deve preservar o comportamento de toda a pilha usada pelo Tedious para falar com SQL Server:

- framing e headers de pacotes TDS;
- fragmentação, reagrupamento, packet ID e indicador de fim;
- PRELOGIN e negociação de encryption;
- TLS, validação de certificado e hostname;
- LOGIN7 e opções de sessão;
- SQL Authentication, NTLM e tokens necessários;
- tokenização incremental de respostas;
- metadados de colunas, rows, DONE, RETURNSTATUS, ERROR e INFO;
- RPC, SQL batch, parâmetros, tipos, nullability, precision, scale e collation;
- cancelamento, timeout, fechamento, half-close e recuperação de erros;
- transações, bulk load, streaming e pooling.

Nunca assumir que uma leitura TCP produz um pacote completo. Nunca assumir que um pacote contém um token completo. Todo comprimento recebido da rede deve ser validado antes de indexar ou alocar memória.

TLS não deve ser desabilitado como correção rápida. Downgrade, certificado inválido, hostname divergente e servidor que exige criptografia devem resultar em comportamento explícito e testado.

## Regras de Rust específicas deste port

As práticas idiomáticas de Rust são secundárias à paridade, mas segurança de memória e comportamento definido são obrigatórios:

- Não usar `unsafe` para contornar o port sem justificativa, encapsulamento e teste.
- Não permitir `panic` em dados recebidos do servidor, entradas do usuário ou falhas de autenticação.
- Preservar ownership de buffers em operações assíncronas; nenhuma referência pode sobreviver ao buffer proprietário.
- Não bloquear o runtime assíncrono com operações de filesystem, DNS ou banco que sejam síncronas.
- Tornar timeouts, cancelamento e shutdown observáveis e determinísticos.
- Manter conversões de tipos SQL explícitas; nunca truncar silenciosamente precisão, escala, bytes ou caracteres.
- Evitar logs com senha, token, connection string completa, dados de linha ou SQL sensível.
- Usar `cargo fmt`, `cargo check`, `cargo clippy` e `cargo test` como verificações mínimas quando aplicáveis, sem transformar lint em motivo para reescrever a lógica equivalente.

## Testes: regra absoluta de localização

Todos os testes, sem exceção, devem ficar em:

`/Users/neverson/myspace/orbitsql/tedious-rs/tests`

Isso inclui testes unitários, testes de protocolo, fixtures executáveis, testes de integração, testes de compatibilidade e helpers de teste. Não colocar `#[cfg(test)]` ou módulos de teste dentro de `src`, salvo se for tecnicamente inevitável e previamente documentado; a regra padrão é manter tudo em `tests`.

Os testes devem ser determinísticos, legíveis e independentes. Segredos nunca podem ser gravados em fixtures, snapshots, logs, código-fonte ou mensagens de erro esperadas.

### Estrutura física obrigatória dos testes Rust

A documentação de testes Rust consultada apresenta testes unitários com `#[cfg(test)]` no mesmo arquivo de produção. Essa organização não se aplica a este projeto: a regra específica deste port prevalece e nenhum teste deve ficar dentro de `src`.

Usar esta estrutura dentro de `tedious-rs`:

```text
tests/
├── common/
│   └── mod.rs
├── fixtures/
│   ├── packets/
│   ├── payloads/
│   ├── tokens/
│   ├── types/
│   └── auth/
├── unit/
│   ├── all_headers.rs
│   ├── buffer.rs
│   ├── collation.rs
│   ├── connection_state.rs
│   ├── data_types.rs
│   ├── guid.rs
│   ├── login7.rs
│   ├── ntlm.rs
│   ├── packet.rs
│   ├── payloads.rs
│   ├── prelogin.rs
│   ├── token_parsers.rs
│   └── value_parser.rs
├── integration/
│   ├── sql_auth_connection.rs
│   ├── metadata.rs
│   ├── crud.rs
│   ├── read_only.rs
│   ├── ntlm_read_only.rs
│   ├── transactions.rs
│   ├── rpc.rs
│   ├── prepared.rs
│   ├── bulk_load.rs
│   ├── tvp.rs
│   ├── datatypes.rs
│   └── streaming.rs
└── compatibility/
    ├── node_fixtures.rs
    └── event_order.rs
```

Regra física obrigatória: nenhuma implementação de teste pode ficar diretamente sob `tests/`. Todo teste unitário, inclusive protocolo, transporte fake, lifecycle, API pública, parsing, segurança e paridade determinística, deve ficar em `tests/unit/`. Todo teste de integração que acessa SQL Server, inclusive SQL Authentication, NTLM, metadados, CRUD, transações, streaming e cenários reais de conexão, deve ficar em `tests/integration/`. Testes que executam o Node ou comparam observações Node/Rust devem ficar em `tests/compatibility/`.

Os únicos arquivos `.rs` permitidos diretamente sob `tests/` são harnesses de registro, atualmente `unit.rs`, `integration.rs` e `compatibility.rs`. Esses arquivos não devem conter funções de teste; devem apenas registrar os módulos por caminho e declarar os helpers comuns necessários. Cada implementação registrada por eles permanece fisicamente dentro de sua subpasta. `tests/common/` contém somente helpers compartilhados. Os diretórios `fixtures/` podem conter somente dados sanitizados; não colocar lógica de teste ou credenciais neles.

Ao mover um teste, atualizar o harness correspondente com `#[path = "unit/arquivo.rs"]`, `#[path = "integration/arquivo.rs"]` ou `#[path = "compatibility/arquivo.rs"]`, conforme a classificação. Não deixar um teste apenas movido sem registro no harness, porque isso pode fazer o Cargo deixar de descobri-lo. Ao mover um arquivo que dependia de `mod common;`, ajustar o caminho para usar o `tests/common/` exposto pelo harness, sem duplicar helpers.

Os testes unitários devem importar a API pública da biblioteca como um consumidor externo. Se um detalhe privado precisar de teste, expor o menor contrato de teste possível na API pública ou criar um adaptador; não mover o teste para `src` apenas para acessar internals.

O `tedious-rs` deve ser uma library crate (`src/lib.rs`) para que os crates de `tests/` possam importá-la. `src/main.rs`, se permanecer, deve ser apenas binário/CLI e não deve conter testes.

### Regras derivadas da documentação de testes Rust

- Usar `#[test]`, `assert!`, `assert_eq!`, `assert_ne!` e testes que retornem `Result` quando isso tornar o erro mais claro.
- Nomear testes pelo comportamento observado, por exemplo `read_done_rejects_truncated_buffer`, não por detalhes de implementação.
- Agrupar cenários relacionados em módulos dentro do arquivo de teste: `valid`, `invalid`, `boundaries`, `errors`, `lifecycle` e `compatibility`.
- Mensagens de assertion devem explicar a divergência sem incluir credenciais, tokens ou payloads sensíveis.
- `#[should_panic]` só pode ser usado para um panic deliberado e controlado; dados de rede nunca devem causar panic.
- `#[ignore]` exige motivo na spec correspondente e não conta como cobertura até passar com `--ignored`.
- Testes assíncronos devem usar o runtime adotado pelo projeto e devem aguardar explicitamente shutdown, EOF e conclusão de tasks.
- Mocks são permitidos para socket, relógio, DNS, provider de chave e credencial; prefira traits/adapters pequenos e valide também com transporte fake que envie bytes reais.
- `mockall` ou crate semelhante só deve ser adicionado após justificar a necessidade e avaliar impacto no port; mock não substitui teste de protocolo nem integração.
- Benchmarks Criterion medem desempenho e não substituem testes. Se forem adicionados, devem possuir configuração explícita e não podem conter credenciais; o contrato funcional continua em `tests/`.

### Doc tests não são permitidos

Doc tests dependem de comentários `///`/`//!`. Como comentários no código são proibidos neste projeto, não escrever doc comments nem usar exemplos executáveis embutidos em código. Exemplos de API, comandos e contratos devem ficar em `specs/`, e exemplos compiláveis devem ser testes externos em `tests/`.

### Testes unitários obrigatórios

Cada módulo portado deve ter testes unitários em `tests`, cobrindo, conforme o módulo:

- parsing e serialização byte a byte;
- pacotes fragmentados, vazios, truncados e com comprimentos inválidos;
- tokenizer e transições de estado;
- tipos TDS, nulls, unicode, collation, precision e scale;
- PRELOGIN, LOGIN7, TLS e autenticação com vetores sanitizados;
- erros SQL, INFO, DONE, return status e múltiplas mensagens;
- parâmetros, RPC, SQL batch, bulk load e transações;
- timeout, cancelamento, concorrência, streaming e fechamento;
- equivalência de defaults, callbacks/eventos e ordem de conclusão.

Não considerar cobertura de linhas como prova de paridade. O teste deve verificar comportamento observável e casos-limite.

### Testes de integração obrigatórios

Os testes de integração devem executar contra um SQL Server local em funcionamento e devem ficar em `tedious-rs/tests`. Eles são obrigatórios para conexão, autenticação, metadados e CRUD.

### Perfil obrigatório: SQL Server Authentication local mutável

Este perfil é obrigatório para os testes de integração de SQL Server Authentication. Não é uma configuração de referência, exemplo ou fallback: a suíte deve conectar usando SQL Authentication tradicional e executar o cenário controlado de teste com DDL, DML, CRUD, transações e leitura de metadados.

- host: `localhost`
- usuário: `sa` ou `SA`
- senha efetiva: `DevStr0ng@Pass` (a notação `DevStr0ng\@Pass` representa essa mesma senha quando o `@` é escapado)
- banco: `tedious_rs`

Esse perfil não usa `TEST_DB_HOST`, `VPN_USER`, `VPN_PWD` ou `TEST_DB_NAME`, que pertencem exclusivamente ao perfil NTLM. Não copiar esses valores para código de produção, logs, fixtures, snapshots ou mensagens de erro.

### Perfil obrigatório: NTLM PROD read-only

Este perfil é exclusivamente NTLM e deve ser tratado como produção. Ele não pode ser usado para SQL Server Authentication tradicional e não pode executar DDL, DML, CRUD mutável, procedures mutáveis, bulk load ou qualquer comando que altere estado. Somente `SELECT` e leitura de metadados são permitidos.

Todos os valores deste perfil devem ser obtidos em runtime pelas variáveis de ambiente abaixo, sem exigir `export` persistente:

- host: `TEST_DB_HOST`
- usuário: `VPN_USER`
- senha: `VPN_PWD`
- banco: `TEST_DB_NAME`
- domínio opcional: `VPN_DOMAIN`

`VPN_DOMAIN` é opcional. A suíte deve testar domínio ausente, domínio vazio e domínio informado quando a variável existir. A ausência de `VPN_DOMAIN` jamais pode impedir a tentativa NTLM quando o servidor aceitar usuário sem domínio.

Os testes de integração devem validar, no mínimo:

- abertura e fechamento de conexão;
- SQL Server Authentication local com usuário e senha tradicionais;
- criação/limpeza controlada do cenário no banco `tedious_rs` usando DDL e DML;
- obtenção de metadados de objetos: tabelas, colunas, tipos, nullability, chaves e procedures quando disponíveis;
- `CREATE`, `INSERT`, `SELECT`, `UPDATE` e `DELETE` em tabelas de teste;
- parâmetros, nulls, unicode, tipos numéricos e temporais;
- múltiplos result sets, erros do servidor e transações;
- comparação do resultado com o comportamento do `tedious-node`.

Os testes do perfil SQL Authentication local podem criar, alterar, inserir, consultar, atualizar, excluir e remover seus próprios objetos. Devem usar nomes identificáveis e limpeza em sucesso e falha.

## Conexão read-only

Deve existir uma configuração e uma suíte de integração para conexão READ ONLY, permitindo apenas `SELECT` e operações de leitura de metadados.

- A suíte read-only nunca pode executar `CREATE`, `ALTER`, `DROP`, `INSERT`, `UPDATE`, `DELETE`, `MERGE`, `TRUNCATE`, `EXEC` mutável ou comandos de controle de sessão que alterem dados.
- A intenção read-only deve ser reforçada pelo usuário/permissão do banco quando disponível e também por uma guarda no cliente/test harness.
- Um teste deve comprovar que uma operação de escrita é rejeitada antes de ser enviada ou pelo SQL Server, conforme a camada responsável.
- Consultas de metadados devem continuar permitidas.
- Falha na detecção de uma operação potencialmente mutável deve ser tratada como bloqueio seguro, nunca como autorização.

## Conexão NTLM

Deve existir uma suíte de integração para NTLM em Linux e macOS. O comportamento deve ser compatível com o Node: o domínio é opcional.

- A conexão deve funcionar com usuário no formato simples, sem domain.
- A conexão deve funcionar com usuário que contenha domain, quando o ambiente fornecer essa informação.
- Não tornar `domain` obrigatório no builder, configuração, handshake ou validação.
- Testar domínio ausente, domínio vazio e domínio informado, sem vazar credenciais.
- Cobrir negociação, challenge/response, flags, encoding e falhas de autenticação.
- Essa conexão deve ser tratada como PROD: somente `SELECT` e leitura de metadados são permitidos.
- O harness deve bloquear qualquer teste mutável nessa configuração e deve verificar que nenhuma operação de escrita foi enviada.

As credenciais NTLM devem vir exclusivamente de `TEST_DB_HOST`, `VPN_USER`, `VPN_PWD`, `TEST_DB_NAME` e, opcionalmente, `VPN_DOMAIN`. Nenhuma dessas variáveis pode ser usada para selecionar ou autenticar o perfil SQL Authentication local.

## Segurança dos testes e do banco

- Nunca imprimir `VPN_PWD`, a senha local, tokens, challenges completos ou connection strings.
- Sanitizar traces e capturas de pacote antes de salvar fixtures.
- Usar tabelas/objetos temporários ou nomes exclusivos para não tocar em dados de produção.
- A configuração NTLM/PROD é read-only por política, mesmo que a conta possua mais permissões.
- Falhar fechado diante de modo de autenticação desconhecido, banco incorreto ou intenção de escrita.
- Não executar comandos destrutivos fora do banco e dos objetos criados pelo próprio teste.

## Fluxo de implementação

Para cada unidade do `tedious-node`:

1. Localizar e ler o arquivo de origem completo e suas dependências diretas.
2. Mapear imports e seguir o código relevante em `node_modules`, incluindo a versão resolvida.
3. Identificar classes, funções, constantes, estados, eventos e efeitos colaterais.
4. Pesquisar crates Rust existentes para cada capacidade delegada e avaliar sua adequação.
5. Criar o módulo Rust correspondente com a mesma decomposição.
6. Portar literalmente o que for lógica própria e encapsular crates para o que for capacidade reutilizável.
7. Adicionar testes unitários em `tedious-rs/tests` antes de refatorar.
8. Validar bytes e comportamento contra fixtures e, quando possível, contra o Node.
9. Adicionar o teste de integração SQL Server para o comportamento que cruza a rede.
10. Registrar cada crate adotado, cada lógica portada e qualquer divergência, sua causa e seu teste de proteção.

Não avançar silenciosamente quando a referência não estiver clara. Marcar a unidade como bloqueada, registrar a dúvida e continuar apenas em uma parte independente.

## Critérios de pronto

Uma unidade só é considerada portada quando:

- sua estrutura e responsabilidades correspondem ao `tedious-node`;
- caminhos de sucesso, erro, cancelamento e fechamento têm testes;
- parsing/serialização possuem fixtures determinísticas;
- testes unitários estão em `tedious-rs/tests`;
- testes de integração relevantes passam no SQL Server local via variáveis de ambiente;
- SQL auth, read-only e NTLM sem domain obrigatório estão cobertos quando aplicáveis;
- não há credenciais hardcoded;
- divergências inevitáveis estão documentadas;
- `cargo fmt`, `cargo check`, `cargo clippy` e os testes aplicáveis foram executados.

Uma versão “estável” do driver exige ainda uma matriz de compatibilidade com o `tedious-node`, incluindo API, eventos, erros, tipos, TDS, SQL Server Authentication, NTLM em Linux/macOS, modo read-only e operações CRUD no banco de integração.

## Proibição absoluta de comentários no código

Comentários são proibidos. Não adicionar comentários de linha, bloco ou documentação em qualquer código do projeto:

- proibidos `//`, `/* ... */`, `///` e `//!` em arquivos `.rs`, incluindo testes;
- proibidos comentários em scripts de build, fixtures executáveis e arquivos de configuração de código;
- proibidos `TODO`, `FIXME`, `XXX` e explicações inline como forma de registrar pendências;
- não copiar comentários do `tedious-node` durante o port;
- não usar doc comments para gerar doc tests.

O código deve ser autoexplicativo por nomes, tipos, funções pequenas e estrutura equivalente à referência. Decisões, mapeamentos, limitações, divergências, dependências e instruções devem ficar em `tedious-rs/specs/` ou no `AGENTS.md`, nunca em comentários dentro do código. Textos de erro e strings que façam parte do comportamento do driver não são comentários e devem ser preservados quando exigidos pela compatibilidade.

Antes de concluir qualquer alteração, verificar que os arquivos de código modificados não contêm marcadores de comentário. Esta proibição não se aplica à documentação Markdown externa, às specs ou ao próprio `AGENTS.md`.
