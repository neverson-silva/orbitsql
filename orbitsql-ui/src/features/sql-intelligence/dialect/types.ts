import type { IdentifierQuoteStyle, SqlTypeFamily } from "../catalog/types";

export type DialectId =
  | "postgresql"
  | "mysql"
  | "mariadb"
  | "sqlserver"
  | "oracle"
  | "sqlite"
  | "generic";

export type ParameterStyle = "$n" | "question" | "at-name" | ":name";

export type CodeMirrorDialectId =
  | "postgresql"
  | "mysql"
  | "mariadb"
  | "sqlserver"
  | "oracle"
  | "sqlite"
  | "standard";

export type SqlDialect = Readonly<{
  id: DialectId;
  codeMirrorDialect: CodeMirrorDialectId;
  identifierRules: Readonly<{
    quoteStyles: readonly IdentifierQuoteStyle[];
    caseFold: "lower" | "upper" | "preserve";
  }>;
  quoting: Readonly<{
    open: string;
    close: string;
    style: IdentifierQuoteStyle;
  }>;
  keywords: ReadonlySet<string>;
  functions: ReadonlyMap<
    string,
    Readonly<{
      name: string;
      returnFamily: SqlTypeFamily;
      argumentFamilies: readonly SqlTypeFamily[];
    }>
  >;
  operators: readonly string[];
  parameterStyle: ParameterStyle;
  capabilities: Readonly<{
    returning: boolean;
    onConflict: boolean;
    output: boolean;
    top: boolean;
    limit: boolean;
    sequences: boolean;
  }>;
}>;
