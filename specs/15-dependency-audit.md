# Auditoria de dependências

As dependências do port são verificadas contra os entrypoints efetivamente resolvidos do Node e contra adaptadores Rust pequenos e testáveis.

| capacidade | referência | adaptação |
| --- | --- | --- |
| codificação | iconv-lite | encoding_rs |
| MD4 | js-md4 | md4 |
| cache | lru-cache | estruturas locais |
| autenticação | @azure/identity e @azure/keyvault-keys | reqwest e adaptadores Azure |
| formatação | sprintf-js | formatação explícita |
| TLS | rustls | adaptador de transporte |
| compatibilidade de texto | yore | normalização de encoding |

Os detalhes observados ficam em `specs/dependency-evidence/15-observations.md`.
