import type { CompletionItem, RankingFeatures } from "./types";

function prefixQuality(label: string, prefix: string): number {
  if (prefix.length === 0) return 0.4;
  const normalizedLabel = label.toLocaleLowerCase();
  const normalizedPrefix = prefix.toLocaleLowerCase();
  const lastPart = normalizedLabel.slice(normalizedLabel.lastIndexOf(".") + 1);
  if (normalizedLabel === normalizedPrefix) return 1;
  if (
    normalizedLabel.startsWith(normalizedPrefix) ||
    lastPart.startsWith(normalizedPrefix)
  )
    return 0.9;
  let cursor = 0;
  for (const character of normalizedLabel) {
    if (character === normalizedPrefix[cursor]) cursor += 1;
    if (cursor === normalizedPrefix.length) return 0.45;
  }
  return 0;
}

function weightedScore(
  item: CompletionItem,
  features: RankingFeatures,
): number {
  return (
    item.score +
    features.prefixQuality * 40 +
    features.semanticFit * 25 +
    features.scopeDistance * 15 +
    features.qualifierMatch * 20 +
    features.typeCompatibility * 15 +
    features.relationshipConfidence * 30 +
    features.objectKindPrior * 5 +
    features.schemaAffinity * 5 +
    features.usageSignal * 5 -
    features.ambiguityPenalty * 15
  );
}

export function rankCompletionItems(
  items: readonly CompletionItem[],
  prefix: string,
): readonly CompletionItem[] {
  const normalizedPrefix = prefix.toLocaleLowerCase();
  const exact = items.filter(
    (item) =>
      item.displayLabel.toLocaleLowerCase().startsWith(normalizedPrefix) ||
      item.displayLabel
        .toLocaleLowerCase()
        .slice(item.displayLabel.lastIndexOf(".") + 1)
        .startsWith(normalizedPrefix) ||
      item.insertText.toLocaleLowerCase().startsWith(normalizedPrefix),
  );
  const candidates =
    exact.length > 0
      ? exact
      : items.filter((item) => prefixQuality(item.displayLabel, prefix) > 0);
  return candidates
    .map((item) => {
      const features: RankingFeatures = {
        prefixQuality: prefixQuality(item.displayLabel, prefix),
        semanticFit: item.features?.semanticFit ?? 0,
        scopeDistance: item.features?.scopeDistance ?? 0,
        qualifierMatch: item.features?.qualifierMatch ?? 0,
        typeCompatibility: item.features?.typeCompatibility ?? 0.5,
        relationshipConfidence: item.features?.relationshipConfidence ?? 0,
        objectKindPrior: item.features?.objectKindPrior ?? 0,
        schemaAffinity: item.features?.schemaAffinity ?? 0,
        usageSignal: item.features?.usageSignal ?? 0,
        ambiguityPenalty: item.features?.ambiguityPenalty ?? 0,
      };
      return { ...item, score: weightedScore(item, features), features };
    })
    .sort(
      (left, right) =>
        right.score - left.score ||
        left.displayLabel.localeCompare(right.displayLabel) ||
        (left.sourceId ?? "").localeCompare(right.sourceId ?? ""),
    );
}
