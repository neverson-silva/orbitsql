import type {
  CompletionContext,
  CompletionItem,
  CompletionRequest,
} from "./types";

export function provideSnippetCandidates(
  _request: CompletionRequest,
  context: CompletionContext,
): readonly CompletionItem[] {
  if (context.kind !== "statement-start" && context.kind !== "keyword")
    return [];
  const prefix = context.prefix.toLocaleLowerCase();
  if (prefix !== "s" && prefix !== "se" && prefix !== "sel") return [];
  const insertText = String.raw`SELECT \${1:column}
FROM \${2:table}`;
  return [
    {
      from: context.range.from,
      to: context.range.to,
      insertText,
      displayLabel: "SELECT query",
      kind: "snippet",
      detail: "SQL query snippet",
      score: 70,
      provenance: "syntax",
      snippet: insertText,
    },
  ];
}
