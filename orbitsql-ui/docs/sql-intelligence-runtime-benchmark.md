# SQL intelligence runtime benchmark

Este experimento compara topologias de execução sem substituir o runtime de produção. Ele usa `worker_threads` do Node e um kernel sintético de scoring sobre um catálogo compacto. O kernel não executa `complete()`; portanto, os resultados medem paralelismo, IPC, ownership de memória e fila, não qualidade semântica.

Execute com:

```text
pnpm bench:runtime
```

Parâmetros:

```text
ORBITSQL_BENCH_OBJECTS=500000
ORBITSQL_BENCH_ROUNDS=24
ORBITSQL_BENCH_SAMPLES=15
```

O experimento cobre:

- main thread;
- um worker com catálogo compartilhado;
- um worker com structured clone;
- pools de 2, 4 e 8 workers;
- fan-out por faixas de catálogo com top-K local;
- coordinator worker que encaminha para quatro workers;
- lane interativa isolada enquanto workers de background estão ocupados;
- atraso de fila e bloqueio do event loop;
- heap reportado por worker.

Resultado observado em Apple Silicon, Node v24.14.1, 500 mil entidades:

| Topologia | p50 | p95 | Heap JS dos workers | Observação |
|---|---:|---:|---:|---|
| Main thread | 8,20 ms | 9,42 ms | — | bloqueia o event loop por ~8,69 ms |
| 1 worker + SAB | 8,09 ms | 13,73 ms | 6,57 MB | remove o trabalho da UI, sem speedup relevante |
| Pool 2 + SAB | 3,81 ms | 3,90 ms | 13,14 MB | primeiro ganho consistente |
| Pool 4 + SAB | 2,15 ms | 2,25 ms | 27,04 MB | melhor compromisso observado |
| Pool 8 + SAB | 1,78 ms | 1,84 ms | 54,65 MB | ganho marginal e maior custo |
| Coordinator + pool 4 | 2,18 ms | 3,46 ms | 6,86 MB no coordinator | segundo salto piora a cauda |
| Pool 4 com clone | não aplicado ao request | — | 54,90 MB | inicialização ~147 ms |

Em um cenário de 500 mil entidades, structured clone de 8 workers consumiu aproximadamente 109,75 MB de heap JS nos workers e levou aproximadamente 483 ms para inicialização. O modo compartilhado usa um buffer de 2 MB para o catálogo sintético, uma vez.

Sob background pesado, enviar completion à mesma lane produziu cerca de 171 ms de espera; uma lane interativa dedicada produziu cerca de 0,09 ms de espera. Essa é a evidência principal para separar capacidade interativa de manutenção de catálogo.

Os pontos ainda não provados por este experimento são afinidade real por conexão, fan-out dos providers reais e execução dentro do WebView Tauri. Esses devem ser medidos depois que o adapter de worker executar o engine real com snapshots reais.
