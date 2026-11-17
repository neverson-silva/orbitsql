import type { DialectId } from "../dialect/types";

export type SqlDocument = Readonly<{
  id: string;
  version: number;
  text: string;
  dialect: DialectId;
}>;

export type TextRange = Readonly<{
  from: number;
  to: number;
}>;
