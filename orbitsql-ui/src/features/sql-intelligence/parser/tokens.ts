import type {
  Identifier,
  IdentifierQuoteStyle,
  QualifiedName,
} from "../catalog/types";
import { normalizeIdentifier } from "../dialect/registry";
import type { SqlDialect } from "../dialect/types";

export type SqlTokenKind =
  | "keyword"
  | "identifier"
  | "quoted-identifier"
  | "string"
  | "number"
  | "parameter"
  | "operator"
  | "punctuation";

export type SqlToken = Readonly<{
  kind: SqlTokenKind;
  text: string;
  normalized: string;
  from: number;
  to: number;
  quoteStyle?: IdentifierQuoteStyle;
}>;

const identifierStart = /[\p{L}_$]/u;
const identifierPart = /[\p{L}\p{N}_$]/u;

function isIdentifierStart(value: string | undefined): boolean {
  return value !== undefined && identifierStart.test(value);
}

function isIdentifierPart(value: string | undefined): boolean {
  return value !== undefined && identifierPart.test(value);
}

function quotedToken(
  text: string,
  from: number,
  to: number,
  quoteStyle: IdentifierQuoteStyle,
  dialect: SqlDialect,
): SqlToken {
  return {
    kind: "quoted-identifier",
    text,
    normalized: normalizeIdentifier(text, dialect),
    from,
    to,
    quoteStyle,
  };
}

function readQuoted(
  text: string,
  start: number,
  open: string,
  close: string,
): { token: string; end: number } {
  let cursor = start + open.length;
  while (cursor < text.length) {
    if (text.startsWith(close, cursor)) {
      if (text.startsWith(close + close, cursor)) {
        cursor += close.length * 2;
        continue;
      }
      cursor += close.length;
      return { token: text.slice(start, cursor), end: cursor };
    }
    cursor += 1;
  }
  return { token: text.slice(start), end: text.length };
}

export function tokenizeSql(
  text: string,
  dialect: SqlDialect,
): readonly SqlToken[] {
  const tokens: SqlToken[] = [];
  let cursor = 0;
  while (cursor < text.length) {
    const current = text[cursor];
    if (/\s/u.test(current)) {
      cursor += 1;
      continue;
    }
    if (text.startsWith("--", cursor)) {
      const lineEnd = text.indexOf("\n", cursor + 2);
      cursor = lineEnd < 0 ? text.length : lineEnd;
      continue;
    }
    if (text.startsWith("/*", cursor)) {
      const blockEnd = text.indexOf("*/", cursor + 2);
      cursor = blockEnd < 0 ? text.length : blockEnd + 2;
      continue;
    }
    if (
      current === "'" ||
      current === '"' ||
      current === "`" ||
      current === "["
    ) {
      const close = current === "[" ? "]" : current;
      const quoteStyle: IdentifierQuoteStyle =
        current === '"'
          ? "double"
          : current === "`"
            ? "backtick"
            : current === "["
              ? "bracket"
              : "none";
      const quoted = readQuoted(text, cursor, current, close);
      if (current === "'") {
        tokens.push({
          kind: "string",
          text: quoted.token,
          normalized: quoted.token,
          from: cursor,
          to: quoted.end,
        });
      } else {
        tokens.push(
          quotedToken(quoted.token, cursor, quoted.end, quoteStyle, dialect),
        );
      }
      cursor = quoted.end;
      continue;
    }
    const parameter =
      dialect.parameterStyle === "$n"
        ? text.slice(cursor).match(/^\$\d+/u)?.[0]
        : dialect.parameterStyle === "at-name"
          ? text.slice(cursor).match(/^@[\p{L}_$][\p{L}\p{N}_$]*/u)?.[0]
          : dialect.parameterStyle === ":name"
            ? text.slice(cursor).match(/^:[\p{L}_$][\p{L}\p{N}_$]*/u)?.[0]
            : current === "?"
              ? "?"
              : undefined;
    if (parameter) {
      tokens.push({
        kind: "parameter",
        text: parameter,
        normalized: parameter,
        from: cursor,
        to: cursor + parameter.length,
      });
      cursor += parameter.length;
      continue;
    }
    if (isIdentifierStart(current)) {
      const start = cursor;
      cursor += 1;
      while (isIdentifierPart(text[cursor])) cursor += 1;
      const tokenText = text.slice(start, cursor);
      const normalized = normalizeIdentifier(tokenText, dialect);
      const kind: SqlTokenKind = dialect.keywords.has(
        normalized.toLocaleLowerCase(),
      )
        ? "keyword"
        : "identifier";
      tokens.push({
        kind,
        text: tokenText,
        normalized,
        from: start,
        to: cursor,
      });
      continue;
    }
    if (/\d/u.test(current)) {
      const start = cursor;
      cursor += 1;
      while (/[\d.]/u.test(text[cursor] ?? "")) cursor += 1;
      tokens.push({
        kind: "number",
        text: text.slice(start, cursor),
        normalized: text.slice(start, cursor),
        from: start,
        to: cursor,
      });
      continue;
    }
    const operator = [...dialect.operators]
      .sort((left, right) => right.length - left.length)
      .find((candidate) => text.startsWith(candidate, cursor));
    if (operator) {
      tokens.push({
        kind: "operator",
        text: operator,
        normalized: operator,
        from: cursor,
        to: cursor + operator.length,
      });
      cursor += operator.length;
      continue;
    }
    const kind: SqlTokenKind = ".,();".includes(current)
      ? "punctuation"
      : "operator";
    tokens.push({
      kind,
      text: current,
      normalized: current,
      from: cursor,
      to: cursor + 1,
    });
    cursor += 1;
  }
  return tokens;
}

export function tokenText(token: SqlToken | undefined): string {
  return token?.normalized.toLocaleLowerCase() ?? "";
}

export function isWordToken(token: SqlToken | undefined): boolean {
  return (
    token?.kind === "identifier" ||
    token?.kind === "quoted-identifier" ||
    token?.kind === "keyword"
  );
}

export function identifierFromToken(token: SqlToken): Identifier {
  return {
    raw: token.text,
    normalized: token.normalized,
    quoted: token.kind === "quoted-identifier",
    quoteStyle: token.quoteStyle ?? "none",
  };
}

export function qualifiedNameFromTokens(
  tokens: readonly SqlToken[],
  start: number,
  end: number,
): { name: QualifiedName; next: number } {
  if (!isWordToken(tokens[start]))
    return {
      name: {
        object: { raw: "", normalized: "", quoted: false, quoteStyle: "none" },
      },
      next: start,
    };
  const parts = [identifierFromToken(tokens[start])];
  let index = start + 1;
  while (
    index + 1 <= end &&
    tokens[index]?.text === "." &&
    isWordToken(tokens[index + 1])
  ) {
    parts.push(identifierFromToken(tokens[index + 1]));
    index += 2;
  }
  const object = parts[parts.length - 1];
  const schema = parts.length >= 2 ? parts[parts.length - 2] : undefined;
  const catalog = parts.length >= 3 ? parts[parts.length - 3] : undefined;
  return { name: { catalog, schema, object }, next: index };
}

export function matchingParen(
  tokens: readonly SqlToken[],
  openIndex: number,
): number | undefined {
  let depth = 0;
  for (let index = openIndex; index < tokens.length; index += 1) {
    if (tokens[index].text === "(") depth += 1;
    if (tokens[index].text === ")") {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  return undefined;
}
