# SQL intelligence performance

O engine de completion é síncrono, puro e recebe um `CatalogIndex` imutável. O adapter CodeMirror usa um worker compartilhado no desktop e mantém o caminho síncrono para ambientes sem Worker. O worker conserva o snapshot do catálogo e o texto do documento, enquanto o editor envia apenas deltas.

As medições locais são executadas com:

```text
pnpm bench -- --run
```

O benchmark cobre catálogos com 20, 500, 5.000 e 20.000 objetos, além de scripts com 10, 100, 1.000 e 10.000 linhas. O catálogo indexado é construído fora da operação medida para separar refresh de metadata do hot path.

Metas da primeira implementação:

- p50 abaixo de 5 ms depois do aquecimento do catálogo;
- p95 abaixo de 20 ms;
- p99 abaixo de 50 ms;
- completion qualificada por alias em baixa unidade de milissegundos;
- zero chamadas IPC, banco e filesystem por request.

No runtime de produção, a UI faz uma única chamada assíncrona por request de completion e não serializa novamente o catálogo ou o documento inteiro. A resposta interativa é limitada a 100 itens; versões de documento e catálogo são verificadas antes de serem exibidas. O experimento de topologias e suas limitações estão em [sql-intelligence-runtime-benchmark.md](./sql-intelligence-runtime-benchmark.md).

Uma mudança de algoritmo só deve ser feita depois de comparar os resultados do benchmark com essas metas e preservar a mesma saída determinística.
