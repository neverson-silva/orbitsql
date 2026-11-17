export const RESULT_FETCH_SIZE = 200;
export const MIN_RESULT_FETCH_SIZE = 1;
export const MAX_RESULT_FETCH_SIZE = 5_000;
export const MAX_RESULT_ROWS = 10_000;
export const RESULT_PREFETCH_THRESHOLD = 40;

export function virtualResultWindow(
  totalRows: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight = 32,
  overscan = 8,
) {
  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
  const end = Math.min(
    totalRows,
    Math.ceil((scrollTop + viewportHeight) / rowHeight) + overscan,
  );
  return { start, end };
}
