# ADR-013 Query execution context picker

## Decisão

O contexto de execução pertence ao `QueryTab` ativo. Ele contém a conexão e as dimensões específicas do engine, como database, catalog, schema, service, container e user.

O picker interativo vive na sidebar, entre o cabeçalho de conexões e a árvore. A toolbar permanece dedicada a Execute, Commit e Rollback. A árvore continua representando navegação e inspeção; selecionar ou expandir uma conexão na árvore não retargeta uma query.

O status bar exibe passivamente a conexão e o contexto do `QueryTab` ativo. Execute e o picker leem e alteram a mesma fonte canônica no store de query tabs.

## Consequências

Trocar de aba troca imediatamente o contexto mostrado na sidebar e no status bar. Trocar o picker altera somente o tab ativo e preserva o texto SQL. Uma query sem conexão exibe um estado explícito sem contexto e continua impedida de executar.

O picker usa as capacidades existentes por dialecto: PostgreSQL expõe database e schema; MySQL, MariaDB e SQL Server expõem database; Oracle expõe service, container e user/schema. Nenhuma hierarquia universal é inventada para a UI.

Novas queries preservam o comportamento existente e herdam a conexão atualmente ativa na navegação ao serem abertas. Contextos dos tabs permanecem em memória, como o store atual de query tabs, e não são persistidos.
