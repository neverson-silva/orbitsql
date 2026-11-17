export function connectionProfileName(
  detail: string | null | undefined,
): string | null {
  const profile = detail?.split(" • ")[0]?.trim();
  return profile || null;
}

export function profileColorHex(
  profileColor: string | null | undefined,
): string {
  if (profileColor === "Production") return "#ef4444";
  if (profileColor === "Testing") return "#f59e0b";
  if (profileColor === "Development") return "#84cc16";
  return /^#[\da-f]{6}$/i.test(profileColor ?? "")
    ? (profileColor as string)
    : "#84cc16";
}
