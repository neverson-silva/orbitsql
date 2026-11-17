import { spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const project = fileURLToPath(new URL("..", import.meta.url));
const workspace = resolve(project, "..");
const password =
  process.env.ORBITSQL_DOCKER_PASSWORD ?? `OrbitSQL-${randomUUID()}`;
const compose = [
  "docker",
  "compose",
  "-f",
  resolve(project, "docker-compose.yml"),
];

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? project,
    env: { ...process.env, ORBITSQL_DOCKER_PASSWORD: password, ...options.env },
    stdio: "inherit",
  });
  if (result.status !== 0)
    throw new Error(`${command} exited with status ${result.status ?? 1}`);
}

function runCapture(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? project,
    env: { ...process.env, ORBITSQL_DOCKER_PASSWORD: password, ...options.env },
    encoding: "utf8",
  });
  process.stdout.write(result.stdout ?? "");
  process.stderr.write(result.stderr ?? "");
  if (result.status !== 0)
    throw new Error(`${command} exited with status ${result.status ?? 1}`);
  return result.stdout ?? "";
}

try {
  run(compose[0], [
    ...compose.slice(1),
    "down",
    "--volumes",
    "--remove-orphans",
  ]);
  run(compose[0], [...compose.slice(1), "up", "--detach", "--wait"]);
  run(
    "cargo",
    [
      "test",
      "-p",
      "orbitsql-ui",
      "--test",
      "orbitsql_ui_database_adapters",
      "--",
      "--ignored",
    ],
    {
      cwd: workspace,
      env: {
        ORBITSQL_TEST_POSTGRES_HOST: "127.0.0.1",
        ORBITSQL_TEST_POSTGRES_PORT: "55432",
        ORBITSQL_TEST_POSTGRES_USER: "orbitsql",
        ORBITSQL_TEST_POSTGRES_PASSWORD: password,
        ORBITSQL_TEST_POSTGRES_DATABASE: "orbitsql_completion",
        ORBITSQL_TEST_MYSQL_HOST: "127.0.0.1",
        ORBITSQL_TEST_MYSQL_PORT: "53306",
        ORBITSQL_TEST_MYSQL_USER: "orbitsql",
        ORBITSQL_TEST_MYSQL_PASSWORD: password,
        ORBITSQL_TEST_MYSQL_DATABASE: "orbitsql_completion",
        ORBITSQL_TEST_MARIADB_HOST: "127.0.0.1",
        ORBITSQL_TEST_MARIADB_PORT: "53307",
        ORBITSQL_TEST_MARIADB_USER: "orbitsql",
        ORBITSQL_TEST_MARIADB_PASSWORD: password,
        ORBITSQL_TEST_MARIADB_DATABASE: "orbitsql_completion",
      },
    },
  );
  run("cargo", ["clean", "--target-dir", "target/llvm-cov-target"], {
    cwd: workspace,
  });
  run(
    "cargo",
    [
      "llvm-cov",
      "-p",
      "orbitsql-ui",
      "--all-targets",
      "--",
      "--include-ignored",
      "--test-threads=1",
    ],
    {
      cwd: workspace,
      env: {
        ORBITSQL_TEST_POSTGRES_HOST: "127.0.0.1",
        ORBITSQL_TEST_POSTGRES_PORT: "55432",
        ORBITSQL_TEST_POSTGRES_USER: "orbitsql",
        ORBITSQL_TEST_POSTGRES_PASSWORD: password,
        ORBITSQL_TEST_POSTGRES_DATABASE: "orbitsql_completion",
        ORBITSQL_TEST_MYSQL_HOST: "127.0.0.1",
        ORBITSQL_TEST_MYSQL_PORT: "53306",
        ORBITSQL_TEST_MYSQL_USER: "orbitsql",
        ORBITSQL_TEST_MYSQL_PASSWORD: password,
        ORBITSQL_TEST_MYSQL_DATABASE: "orbitsql_completion",
        ORBITSQL_TEST_MARIADB_HOST: "127.0.0.1",
        ORBITSQL_TEST_MARIADB_PORT: "53307",
        ORBITSQL_TEST_MARIADB_USER: "orbitsql",
        ORBITSQL_TEST_MARIADB_PASSWORD: password,
        ORBITSQL_TEST_MARIADB_DATABASE: "orbitsql_completion",
      },
    },
  );
  run(
    "cargo",
    [
      "llvm-cov",
      "report",
      "--summary-only",
      "--fail-under-lines",
      "95",
      "--fail-under-file-lines",
      "95",
      "--fail-under-functions",
      "95",
      "--ignore-filename-regex",
      "(/tedious-rs/|/src-tauri/src/(lib|main|backend/commands)\\.rs)",
    ],
    { cwd: workspace },
  );
  run(
    "cargo",
    ["+nightly", "clean", "--target-dir", "target/llvm-cov-target"],
    {
      cwd: workspace,
    },
  );
  run(
    "cargo",
    [
      "+nightly",
      "llvm-cov",
      "-p",
      "orbitsql-ui",
      "--all-targets",
      "--branch",
      "--",
      "--include-ignored",
      "--test-threads=1",
    ],
    {
      cwd: workspace,
      env: {
        ORBITSQL_TEST_POSTGRES_HOST: "127.0.0.1",
        ORBITSQL_TEST_POSTGRES_PORT: "55432",
        ORBITSQL_TEST_POSTGRES_USER: "orbitsql",
        ORBITSQL_TEST_POSTGRES_PASSWORD: password,
        ORBITSQL_TEST_POSTGRES_DATABASE: "orbitsql_completion",
        ORBITSQL_TEST_MYSQL_HOST: "127.0.0.1",
        ORBITSQL_TEST_MYSQL_PORT: "53306",
        ORBITSQL_TEST_MYSQL_USER: "orbitsql",
        ORBITSQL_TEST_MYSQL_PASSWORD: password,
        ORBITSQL_TEST_MYSQL_DATABASE: "orbitsql_completion",
        ORBITSQL_TEST_MARIADB_HOST: "127.0.0.1",
        ORBITSQL_TEST_MARIADB_PORT: "53307",
        ORBITSQL_TEST_MARIADB_USER: "orbitsql",
        ORBITSQL_TEST_MARIADB_PASSWORD: password,
        ORBITSQL_TEST_MARIADB_DATABASE: "orbitsql_completion",
      },
    },
  );
  const branchReport = runCapture(
    "cargo",
    [
      "+nightly",
      "llvm-cov",
      "report",
      "--summary-only",
      "--ignore-filename-regex",
      "(/tedious-rs/|/src-tauri/src/(lib|main|backend/commands)\\.rs)",
    ],
    { cwd: workspace },
  );
  for (const file of ["mysql.rs", "postgresql.rs"]) {
    const line = branchReport
      .split("\n")
      .find((value) => value.includes(`backend/adapters/drivers/${file}`));
    const percentages = line?.match(/\d+\.\d+%/g);
    const branchCoverage = percentages?.at(-1);
    if (!branchCoverage || Number.parseFloat(branchCoverage) < 95)
      throw new Error(`branch coverage below 95% for ${file}`);
  }
} finally {
  spawnSync(
    compose[0],
    [...compose.slice(1), "down", "--volumes", "--remove-orphans"],
    {
      cwd: project,
      env: { ...process.env, ORBITSQL_DOCKER_PASSWORD: password },
      stdio: "inherit",
    },
  );
}
