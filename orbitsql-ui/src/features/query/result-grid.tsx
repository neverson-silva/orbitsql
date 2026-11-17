import {
  type KeyboardEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { Icon } from "../../components/ui/icons";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "../../components/ui/tooltip";
import { Button, IconButton } from "../../components/ui/ui";
import type { QueryColumn, QueryResult } from "../connections/connection-ipc";
import { profileColorHex } from "../connections/profile-style";
import {
  MAX_RESULT_FETCH_SIZE,
  MIN_RESULT_FETCH_SIZE,
  RESULT_PREFETCH_THRESHOLD,
  virtualResultWindow,
} from "./result-session";

type ResultGridProps = {
  result: QueryResult | null;
  errorMessage: string | null;
  running: boolean;
  resultTab: string;
  onResultTabChange: (value: string) => void;
  onRefresh: () => void;
  onCancel: () => void;
  onLoadMore: () => void;
  batchSize: number;
  onBatchSizeChange: (value: number) => void;
  loadingMore: boolean;
  profile: string | null;
  profileColor?: string;
};

type CellPosition = {
  row: number;
  column: number;
};

const resultTabs = ["Results 1"] as const;

function isNumericColumn(column: QueryColumn) {
  return /int|numeric|decimal|real|double|float|money|serial/i.test(
    column.typeName,
  );
}

function isBooleanColumn(column: QueryColumn) {
  return /bool/i.test(column.typeName);
}

function columnTypeIcon(column: QueryColumn) {
  const typeName = column.typeName.toLowerCase();
  if (typeName.includes("uuid")) return "type-uuid" as const;
  if (/json|jsonb|xml|struct|object/.test(typeName))
    return "type-json" as const;
  if (/\[\]|array|set/.test(typeName)) return "type-array" as const;
  if (/blob|binary|bytea|varbinary|image|raw|bit varying/.test(typeName)) {
    return "type-binary" as const;
  }
  if (/bool|boolean/.test(typeName)) return "type-boolean" as const;
  if (/date|time|timestamp|datetime|year|interval/.test(typeName)) {
    return "type-date" as const;
  }
  if (
    /int|numeric|decimal|real|double|float|money|serial|number/.test(typeName)
  ) {
    return "type-number" as const;
  }
  return "type-text" as const;
}

function columnSizingClass(column: QueryColumn) {
  if (column.name.toLowerCase() === "id") return "min-w-64";
  if (isNumericColumn(column) || isBooleanColumn(column)) return "min-w-24";
  if (/key|name|text|char|uuid/i.test(`${column.name} ${column.typeName}`)) {
    return "min-w-44";
  }
  return "min-w-32";
}

function formatCell(value: unknown): string {
  if (value === null || value === undefined) return "NULL";
  if (typeof value === "string") return value;
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function isJsonColumn(column: QueryColumn) {
  return /json|jsonb/i.test(column.typeName);
}

function formatJsonPreview(value: unknown) {
  if (typeof value !== "string") {
    return JSON.stringify(value, null, 2) ?? formatCell(value);
  }
  try {
    return JSON.stringify(JSON.parse(value) as unknown, null, 2) ?? value;
  } catch {
    return value;
  }
}

function jsonTokens(value: string) {
  const pattern =
    /"(?:\\.|[^"\\])*"|-?\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b|\b(?:true|false|null)\b|[{}\[\],:]/g;
  const tokens: Array<{ text: string; kind: string }> = [];
  let position = 0;
  for (const match of value.matchAll(pattern)) {
    const index = match.index ?? 0;
    if (index > position) {
      tokens.push({ text: value.slice(position, index), kind: "plain" });
    }
    const token = match[0];
    const after = value.slice(index + token.length);
    const kind = token.startsWith('"')
      ? /^\s*:/.test(after)
        ? "key"
        : "string"
      : /^(true|false|null)$/.test(token)
        ? "literal"
        : /^-?\d/.test(token)
          ? "number"
          : "punctuation";
    tokens.push({ text: token, kind });
    position = index + token.length;
  }
  if (position < value.length) {
    tokens.push({ text: value.slice(position), kind: "plain" });
  }
  return tokens;
}

function JsonCellPreview({ value }: { value: unknown }) {
  const formatted = formatJsonPreview(value);
  const tokenClasses: Record<string, string> = {
    key: "text-primary",
    string: "text-success",
    number: "text-warning",
    literal: "text-secondary",
    punctuation: "text-on-surface-variant",
    plain: "text-on-surface",
  };

  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <span
            className="block max-w-80 cursor-zoom-in truncate focus-visible:outline focus-visible:outline-1 focus-visible:outline-primary"
            tabIndex={0}
          >
            {formatCell(value)}
          </span>
        }
      />
      <TooltipContent
        align="start"
        className="max-w-[min(48rem,calc(100vw-2rem))] flex-col items-stretch overflow-hidden border border-floating-border bg-surface-floating p-0 text-on-surface shadow-xl"
        side="bottom"
        sideOffset={6}
      >
        <div className="relative">
          <IconButton
            className="absolute top-2 right-2 z-10 border border-outline-variant/60 bg-surface-floating text-on-surface-variant hover:text-on-surface"
            label="Copy JSON"
            name="copy"
            onClick={() => copyText(formatted)}
          />
          <pre className="max-h-80 max-w-[min(48rem,calc(100vw-2rem))] overflow-auto px-3 pt-10 pb-2 font-mono text-xs leading-5">
            {jsonTokens(formatted).map((token, index) => (
              <span
                className={tokenClasses[token.kind]}
                key={`${index}-${token.kind}`}
              >
                {token.text}
              </span>
            ))}
          </pre>
        </div>
      </TooltipContent>
    </Tooltip>
  );
}

function copyText(value: string) {
  void navigator.clipboard?.writeText(value);
}

export function ResultGrid({
  result,
  errorMessage,
  running,
  resultTab,
  onResultTabChange,
  onRefresh,
  onCancel,
  onLoadMore,
  batchSize,
  onBatchSizeChange,
  loadingMore,
  profile,
  profileColor,
}: ResultGridProps) {
  const [filter, setFilter] = useState("");
  const [filterOpen, setFilterOpen] = useState(false);
  const [focusedCell, setFocusedCell] = useState<CellPosition | null>(null);
  const [selectedRow, setSelectedRow] = useState<number | null>(null);
  const [selectedColumn, setSelectedColumn] = useState<number | null>(null);
  const [scrollState, setScrollState] = useState({
    top: 0,
    height: 480,
  });
  const resultViewportRef = useRef<HTMLDivElement>(null);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    value: string;
    row: number;
    column: number;
  } | null>(null);

  useEffect(() => {
    if (!contextMenu) return;
    const close = () => setContextMenu(null);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [contextMenu]);

  const visibleRows = useMemo(() => {
    if (!result || !filter.trim()) return result?.rows ?? [];
    const query = filter.toLocaleLowerCase();
    return result.rows.filter((row) =>
      row.some((cell) => formatCell(cell).toLocaleLowerCase().includes(query)),
    );
  }, [filter, result]);

  const rowHeight = 32;
  const { start: virtualStart, end: virtualEnd } = virtualResultWindow(
    visibleRows.length,
    scrollState.top,
    scrollState.height,
    rowHeight,
  );
  const virtualRows = visibleRows.slice(virtualStart, virtualEnd);
  const resultIdentity = result
    ? `${result.sessionId ?? "completed"}:${result.elapsedMs}`
    : "empty";

  useEffect(() => {
    if (!resultIdentity) return;
    setScrollState({ top: 0, height: 480 });
    resultViewportRef.current?.scrollTo({ top: 0 });
  }, [resultIdentity]);

  useEffect(() => {
    if (
      running ||
      !result?.hasMore ||
      loadingMore ||
      virtualEnd < Math.max(0, visibleRows.length - RESULT_PREFETCH_THRESHOLD)
    ) {
      return;
    }
    onLoadMore();
  }, [
    loadingMore,
    onLoadMore,
    running,
    result?.hasMore,
    virtualEnd,
    visibleRows.length,
  ]);

  function handleResultScroll(event: React.UIEvent<HTMLDivElement>) {
    const viewport = event.currentTarget;
    setScrollState({ top: viewport.scrollTop, height: viewport.clientHeight });
  }

  const selectedCellCount =
    focusedCell && result
      ? 1
      : selectedRow !== null && result
        ? result.columns.length
        : 0;

  function selectCell(row: number, column: number) {
    setFocusedCell({ row, column });
    setSelectedRow(null);
    setSelectedColumn(null);
  }

  function selectRow(row: number) {
    setSelectedRow(row);
    setSelectedColumn(null);
    setFocusedCell(null);
  }

  function selectColumn(column: number) {
    setSelectedColumn(column);
    setSelectedRow(null);
    setFocusedCell(null);
  }

  function handleGridKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (!result || !focusedCell) return;
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "c") {
      event.preventDefault();
      copyText(formatCell(visibleRows[focusedCell.row]?.[focusedCell.column]));
      return;
    }
    const lastRow = visibleRows.length - 1;
    const lastColumn = result.columns.length - 1;
    let next = focusedCell;
    if (event.key === "ArrowDown")
      next = { ...focusedCell, row: Math.min(lastRow, focusedCell.row + 1) };
    if (event.key === "ArrowUp")
      next = { ...focusedCell, row: Math.max(0, focusedCell.row - 1) };
    if (event.key === "ArrowRight" || event.key === "Tab")
      next = {
        ...focusedCell,
        column: Math.min(lastColumn, focusedCell.column + 1),
      };
    if (event.key === "ArrowLeft" || (event.key === "Tab" && event.shiftKey))
      next = { ...focusedCell, column: Math.max(0, focusedCell.column - 1) };
    if (next !== focusedCell) {
      event.preventDefault();
      setFocusedCell(next);
    }
  }

  return (
    <section
      className="flex min-h-0 flex-1 flex-col overflow-hidden rounded border-2 border-outline-variant/60 bg-app-content"
      aria-label="Query results"
      style={
        profile
          ? {
              borderColor: `color-mix(in srgb, color-mix(in srgb, var(--ds-color-outline-variant) 60%, transparent) 50%, color-mix(in srgb, ${profileColorHex(profileColor)} 60%, transparent) 50%)`,
            }
          : undefined
      }
    >
      <div className="flex min-h-10 shrink-0 items-center gap-1 border-b border-surface-divider bg-app-workspace-subtle px-2">
        <div
          className="flex h-full items-center gap-1"
          role="tablist"
          aria-label="Result sets"
        >
          {resultTabs.map((tab) => (
            <button
              aria-selected={resultTab === tab || resultTab === "Result View"}
              className={`h-full border-b-2 px-2 font-mono text-label-caps transition-colors ${resultTab === tab || resultTab === "Result View" ? "border-primary text-on-surface" : "border-transparent text-on-surface-variant hover:text-on-surface"}`}
              key={tab}
              onClick={() => onResultTabChange(tab)}
              role="tab"
              type="button"
            >
              {tab}
            </button>
          ))}
        </div>
        <span className="font-mono text-label-caps text-on-surface-variant">
          {running
            ? "Executing..."
            : errorMessage
              ? "Query failed"
              : result
                ? result.rowCount > 0
                  ? `${result.rowCount}${result.rowLimitReached ? " rows · limit reached" : result.hasMore ? "+ rows" : " rows"} · ${result.elapsedMs} ms`
                  : `No rows · ${result.elapsedMs} ms`
                : "No results"}
        </span>
        {result?.hasMore ? (
          <span className="font-mono text-label-caps text-on-surface-variant">
            {result.totalFetched.toLocaleString()}+ loaded
          </span>
        ) : null}
        {loadingMore ? (
          <span className="font-mono text-label-caps text-primary">
            Loading more...
          </span>
        ) : null}
        {running ? (
          <Button className="h-7 px-2" onClick={onCancel} variant="danger">
            Cancel
          </Button>
        ) : null}
        <div className="ml-auto flex items-center gap-1">
          <label
            className="flex h-7 items-center gap-1.5 border-l border-surface-divider pl-2 font-mono text-label-caps text-on-surface-variant"
            htmlFor="result-batch-size"
            title="Rows fetched per request"
          >
            <span>Batch</span>
            <input
              aria-label="Rows per result batch"
              className="h-6 w-14 rounded border border-control-border bg-control-surface px-1.5 text-right text-on-surface outline-none transition-colors focus:border-control-border-focus focus:ring-1 focus:ring-control-border-focus/30"
              id="result-batch-size"
              max={MAX_RESULT_FETCH_SIZE}
              min={MIN_RESULT_FETCH_SIZE}
              onChange={(event) => {
                const nextValue = Number.parseInt(event.target.value, 10);
                if (
                  Number.isInteger(nextValue) &&
                  nextValue >= MIN_RESULT_FETCH_SIZE &&
                  nextValue <= MAX_RESULT_FETCH_SIZE
                ) {
                  onBatchSizeChange(nextValue);
                }
              }}
              step={1}
              type="number"
              value={batchSize}
            />
            <span>rows</span>
          </label>
          {filterOpen ? (
            <input
              className="h-7 w-40 border border-control-border bg-control-surface px-2 font-mono text-label-caps text-on-surface outline-none placeholder:text-control-placeholder focus:border-control-border-focus"
              onChange={(event) => setFilter(event.target.value)}
              placeholder="Filter results..."
              value={filter}
            />
          ) : null}
          <IconButton
            label="Filter results"
            name="filter"
            onClick={() => setFilterOpen((open) => !open)}
          />
          <IconButton
            label="Refresh results"
            name="refresh"
            onClick={onRefresh}
          />
          <IconButton label="Export results" name="download" />
          <IconButton label="More result actions" name="more" />
        </div>
      </div>

      {errorMessage ? (
        <div
          className="flex min-h-0 flex-1 items-start justify-start overflow-auto border-t border-error/20 px-4 py-4 text-body-sm text-error"
          role="alert"
        >
          <div className="max-w-4xl border-l-2 border-error/70 pl-3">
            <p className="font-mono text-label-caps text-error">
              Execution error
            </p>
            <p className="mt-1 whitespace-pre-wrap text-on-surface-variant">
              {errorMessage}
            </p>
          </div>
        </div>
      ) : result && result.columns.length > 0 ? (
        <div
          className="relative min-h-0 flex-1 overflow-auto [scrollbar-color:var(--ds-color-surface-container-high)_transparent] [scrollbar-width:thin]"
          data-virtualized-result-grid="true"
          onScroll={handleResultScroll}
          ref={resultViewportRef}
        >
          <table
            className="w-full min-w-[960px] border-collapse text-left font-mono text-label-caps whitespace-nowrap"
            onKeyDown={handleGridKeyDown}
          >
            <thead className="sticky top-0 z-[1] bg-surface-container-low text-on-surface/85">
              <tr className="border-b border-outline-variant/50">
                <th
                  className="sticky left-0 z-[2] w-12 border-r border-outline-variant/60 bg-surface-container-low px-2 py-2 text-right font-medium text-on-surface-variant"
                  scope="col"
                >
                  #
                </th>
                {result.columns.map((column, columnIndex) => (
                  <th
                    aria-selected={selectedColumn === columnIndex}
                    className={`group border-r border-outline-variant/60 px-3 py-2 font-medium ${columnSizingClass(column)} ${selectedColumn === columnIndex ? "bg-selected-surface text-on-surface" : "text-on-surface/85"}`}
                    key={column.name}
                    onClick={() => selectColumn(columnIndex)}
                    scope="col"
                  >
                    <button
                      className="flex w-full cursor-col-resize items-center justify-between gap-2 text-left"
                      type="button"
                    >
                      <span className="flex min-w-0 items-center gap-1.5 truncate">
                        <Icon
                          className="shrink-0 text-on-surface-variant"
                          name={columnTypeIcon(column)}
                          size={12}
                        />
                        <span
                          className="truncate"
                          title={`${column.name} · ${column.typeName}`}
                        >
                          {column.name}
                        </span>
                      </span>
                      <span
                        className="sr-only"
                        title={`${column.name} · ${column.typeName}`}
                      >
                        {column.typeName}
                      </span>
                      <Icon
                        name="chevron-down"
                        size={12}
                        className="invisible shrink-0 text-on-surface-variant group-hover:visible"
                      />
                    </button>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="bg-app-editor">
              {visibleRows.length === 0 ? (
                <tr>
                  <th
                    className="sticky left-0 z-[1] w-12 border-r border-outline-variant/60 border-b-2 border-outline p-0"
                    scope="row"
                  />
                  {result.columns.map((column) => (
                    <td
                      className={`h-7 border-r border-outline-variant/60 border-b-2 border-outline p-0 ${columnSizingClass(column)}`}
                      key={column.name}
                    />
                  ))}
                </tr>
              ) : (
                <>
                  <tr
                    className="pointer-events-none"
                    data-virtual-spacer="true"
                  >
                    <td
                      colSpan={result.columns.length + 1}
                      style={{ height: virtualStart * rowHeight }}
                    />
                  </tr>
                  {virtualRows.map((row, rowOffset) => {
                    const rowIndex = virtualStart + rowOffset;
                    return (
                      <tr
                        aria-rowindex={rowIndex + 1}
                        aria-selected={selectedRow === rowIndex}
                        className={`border-b border-outline-variant/30 hover:bg-control-surface-hover ${selectedRow === rowIndex ? "bg-selected-surface/70" : ""}`}
                        key={JSON.stringify(row)}
                      >
                        <th
                          className={`sticky left-0 z-[1] w-12 cursor-pointer border-r border-outline-variant/60 px-2 py-2 text-right font-normal ${selectedRow === rowIndex ? "bg-selected-surface text-on-surface" : "bg-app-editor text-on-surface-variant"}`}
                          onClick={() => selectRow(rowIndex)}
                          scope="row"
                        >
                          {rowIndex + 1}
                        </th>
                        {result.columns.map((column, columnIndex) => {
                          const value = row[columnIndex];
                          const isFocused =
                            focusedCell?.row === rowIndex &&
                            focusedCell.column === columnIndex;
                          return (
                            <td
                              className={`max-w-80 truncate border-r border-outline-variant/60 px-3 py-2 whitespace-nowrap ${columnSizingClass(column)} ${isNumericColumn(column) ? "text-right" : isBooleanColumn(column) ? "text-center" : "text-left"} ${isFocused ? "bg-control-surface-focus outline outline-1 -outline-offset-1 outline-control-border-focus" : selectedColumn === columnIndex ? "bg-selected-surface/40" : ""} ${value === null || value === undefined ? "italic text-on-surface-variant" : "text-on-surface"}`}
                              key={`${column.name}-${formatCell(value)}`}
                              onClick={() => selectCell(rowIndex, columnIndex)}
                              onContextMenu={(event) => {
                                event.preventDefault();
                                selectCell(rowIndex, columnIndex);
                                setContextMenu({
                                  x: event.clientX,
                                  y: event.clientY,
                                  value: formatCell(value),
                                  row: rowIndex,
                                  column: columnIndex,
                                });
                              }}
                              onKeyDown={(event) => {
                                if (
                                  event.key === "Enter" ||
                                  event.key === " "
                                ) {
                                  event.preventDefault();
                                  selectCell(rowIndex, columnIndex);
                                }
                              }}
                              title={
                                isJsonColumn(column)
                                  ? undefined
                                  : formatCell(value)
                              }
                            >
                              {isJsonColumn(column) && value !== null ? (
                                <JsonCellPreview value={value} />
                              ) : (
                                formatCell(value)
                              )}
                            </td>
                          );
                        })}
                      </tr>
                    );
                  })}
                  <tr
                    className="pointer-events-none"
                    data-virtual-spacer="true"
                  >
                    <td
                      colSpan={result.columns.length + 1}
                      style={{
                        height: (visibleRows.length - virtualEnd) * rowHeight,
                      }}
                    />
                  </tr>
                </>
              )}
            </tbody>
          </table>
          {contextMenu ? (
            <div
              className="fixed z-50 min-w-36 border border-floating-border bg-surface-floating py-1 text-label-caps text-on-surface shadow-lg"
              onClick={(event) => event.stopPropagation()}
              onKeyDown={(event) => event.stopPropagation()}
              role="menu"
              style={{ left: contextMenu.x, top: contextMenu.y }}
              tabIndex={-1}
            >
              <button
                className="block w-full px-3 py-1.5 text-left hover:bg-control-surface-hover"
                onClick={() => copyText(contextMenu.value)}
                type="button"
              >
                Copy value
              </button>
              <button
                className="block w-full px-3 py-1.5 text-left hover:bg-control-surface-hover"
                onClick={() =>
                  copyText(JSON.stringify(visibleRows[contextMenu.row]))
                }
                type="button"
              >
                Copy row
              </button>
              <button
                className="block w-full px-3 py-1.5 text-left hover:bg-control-surface-hover"
                onClick={() => setContextMenu(null)}
                type="button"
              >
                Close
              </button>
            </div>
          ) : null}
          {loadingMore ? (
            <div
              aria-live="polite"
              className="pointer-events-none absolute bottom-3 left-1/2 z-[3] flex -translate-x-1/2 items-center gap-2 rounded-full border border-floating-border bg-floating-surface px-3 py-1.5 font-mono text-label-caps text-on-surface shadow-lg"
            >
              <span
                aria-hidden="true"
                className="size-3 animate-spin rounded-full border-2 border-on-surface/25 border-t-primary"
              />
              Loading more rows...
            </div>
          ) : null}
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 items-center justify-center px-4 text-body-sm text-on-surface-variant">
          {running
            ? "Executing query..."
            : result
              ? "No result set"
              : "Run a query to see live results."}
        </div>
      )}

      <div className="flex h-8 shrink-0 items-center border-t border-outline-variant/50 px-3 font-mono text-label-caps text-on-surface-variant">
        {selectedCellCount > 0
          ? `${selectedCellCount} ${selectedCellCount === 1 ? "cell" : "cells"} selected`
          : result
            ? visibleRows.length > 0
              ? `${visibleRows.length} rows`
              : "No rows"
            : errorMessage
              ? "Execution failed"
              : "No query executed"}
      </div>
    </section>
  );
}
