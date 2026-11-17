# ADR-001 Three-layer architecture

## Decisão

A inteligência SQL é separada em Rust/database foundation, TypeScript SQL intelligence core e CodeMirror/React adapter.

## Consequências

Rust conhece fatos do banco, TypeScript interpreta SQL em snapshots imutáveis e React apenas hospeda o editor. A completion não depende de Tauri, DOM ou banco.
