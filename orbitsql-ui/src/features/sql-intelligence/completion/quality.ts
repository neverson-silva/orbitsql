import type { CompletionItem } from "./types";

export type RankingQualityMetrics = Readonly<{
  top1: number;
  mrr: number;
  recallAt5: number;
  recallAt10: number;
  ndcgAt10: number;
}>;

function reciprocalRank(
  labels: readonly string[],
  expected: ReadonlySet<string>,
): number {
  const index = labels.findIndex((label) => expected.has(label));
  return index < 0 ? 0 : 1 / (index + 1);
}

function recallAt(
  labels: readonly string[],
  expected: ReadonlySet<string>,
  limit: number,
): number {
  if (expected.size === 0) return 1;
  const found = labels.slice(0, limit).filter((label) => expected.has(label));
  return found.length / expected.size;
}

function ndcgAt10(
  labels: readonly string[],
  expected: ReadonlySet<string>,
): number {
  if (expected.size === 0) return 1;
  let dcg = 0;
  for (let index = 0; index < Math.min(10, labels.length); index += 1) {
    if (expected.has(labels[index])) dcg += 1 / Math.log2(index + 2);
  }
  let ideal = 0;
  for (let index = 0; index < Math.min(10, expected.size); index += 1)
    ideal += 1 / Math.log2(index + 2);
  return ideal === 0 ? 0 : dcg / ideal;
}

export function evaluateRankingQuality(
  items: readonly CompletionItem[],
  expectedLabels: readonly string[],
): RankingQualityMetrics {
  const expected = new Set(expectedLabels);
  const labels = items.map((item) => item.displayLabel);
  return {
    top1: expected.has(labels[0] ?? "") ? 1 : 0,
    mrr: reciprocalRank(labels, expected),
    recallAt5: recallAt(labels, expected, 5),
    recallAt10: recallAt(labels, expected, 10),
    ndcgAt10: ndcgAt10(labels, expected),
  };
}
