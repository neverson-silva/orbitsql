import type { TextRange } from "./types";

export function isValidRange(range: TextRange, length: number): boolean {
  return (
    Number.isInteger(range.from) &&
    Number.isInteger(range.to) &&
    range.from >= 0 &&
    range.from <= range.to &&
    range.to <= length
  );
}
