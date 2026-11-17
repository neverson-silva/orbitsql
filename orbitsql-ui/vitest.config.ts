import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: [
      "src/sql-intelligence/tests/**/*.test.ts",
      "src/features/sql-intelligence/tests/**/*.test.ts",
      "src/editor/**/*.test.ts",
      "src/features/editor/**/*.test.ts",
      "src/features/query/**/*.test.ts",
      "src/features/database/**/*.test.ts",
      "src/stores/**/*.test.ts",
      "src/design-system/**/*.test.ts",
      "src/commands/**/*.test.ts",
      "src/components/**/*.test.ts",
    ],
    coverage: {
      provider: "v8",
      include: ["src/features/sql-intelligence/**/*.ts"],
      exclude: ["**/*.test.ts", "**/*.bench.ts"],
      thresholds: {
        branches: 95,
        functions: 95,
        lines: 95,
        statements: 95,
      },
    },
  },
});
