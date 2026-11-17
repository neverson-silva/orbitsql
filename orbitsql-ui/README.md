# OrbitSQL UI

Desktop UI shell for OrbitSQL.

## Stack

- Tauri 2
- Rust crate in `src-tauri`
- React and TypeScript 7
- Vite
- TanStack Router with file-based routes
- Zustand with Tauri Store persistence
- Tailwind CSS 4
- Biome

## Commands

```text
pnpm install
pnpm dev
pnpm build
pnpm check
pnpm typecheck
pnpm test:coverage
pnpm test:rust
pnpm test:rust:providers
pnpm tauri dev
```

The generated route tree is written to `src/routeTree.gen.ts` by the TanStack Router Vite plugin.

`pnpm test:rust:providers` starts PostgreSQL, MySQL 8.4 and MariaDB in Docker with an ephemeral password, runs the six real catalog introspection tests, enforces stable line/function coverage plus nightly branch coverage for the providers, and removes the containers, volumes and network on exit. Install nightly with `llvm-tools-preview` before running this gate.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
