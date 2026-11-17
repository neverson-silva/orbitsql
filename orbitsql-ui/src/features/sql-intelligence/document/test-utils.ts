export function cursorFromMarker(
  text: string,
  marker = "|",
): { text: string; position: number } {
  const position = text.indexOf(marker);
  if (position < 0 || text.indexOf(marker, position + marker.length) >= 0) {
    throw new Error("Exactly one cursor marker is required");
  }
  return {
    text: text.slice(0, position) + text.slice(position + marker.length),
    position,
  };
}
