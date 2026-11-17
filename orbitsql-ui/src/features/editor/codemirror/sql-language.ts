import {
  MariaSQL,
  MSSQL,
  MySQL,
  PLSQL,
  PostgreSQL,
  type SQLDialect,
  SQLite,
  StandardSQL,
} from "@codemirror/lang-sql";
import { getDialect } from "../../sql-intelligence/dialect/registry";
import type {
  CodeMirrorDialectId,
  DialectId,
} from "../../sql-intelligence/dialect/types";

export function codeMirrorDialect(dialect: DialectId): SQLDialect {
  const dialectMap: Record<CodeMirrorDialectId, SQLDialect> = {
    postgresql: PostgreSQL,
    mysql: MySQL,
    mariadb: MariaSQL,
    sqlserver: MSSQL,
    oracle: PLSQL,
    sqlite: SQLite,
    standard: StandardSQL,
  };

  return dialectMap[getDialect(dialect).codeMirrorDialect];
}
