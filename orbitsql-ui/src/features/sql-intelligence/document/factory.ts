import type { DialectId } from "../dialect/types";
import type { SqlDocument } from "./types";

export function createSqlDocument(
  id: string,
  version: number,
  text: string,
  dialect: DialectId,
): SqlDocument {
  return { id, version, text, dialect };
}
