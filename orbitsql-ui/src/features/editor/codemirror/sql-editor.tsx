import { autocompletion } from "@codemirror/autocomplete";
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
  redo,
  undo,
} from "@codemirror/commands";
import { sql } from "@codemirror/lang-sql";
import { EditorState } from "@codemirror/state";
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { useEffect, useRef } from "react";
import {
  createCodeMirrorThemeForId,
  themeCompartment,
} from "../../../design-system";
import { useSettingsStore } from "../../../stores/settings.store";
import { getPlatform } from "../../../window/platform";
import type { CatalogIndex } from "../../sql-intelligence/catalog";
import type { CompletionExecutionContext } from "../../sql-intelligence/completion/types";
import { createSqlCompletionSource } from "./sql-completion-source";
import { createSqlIntelligenceRuntime } from "./sql-intelligence-runtime";
import { codeMirrorDialect } from "./sql-language";

type SqlEditorProps = Readonly<{
  catalog: CatalogIndex;
  documentId: string;
  executionContext?: CompletionExecutionContext;
  value: string;
  onChange: (value: string) => void;
  onSelectionChange?: (selection: { from: number; to: number }) => void;
  className?: string;
  disabled?: boolean;
}>;

export function SqlEditor({
  catalog,
  documentId,
  executionContext,
  value,
  onChange,
  onSelectionChange,
  className,
  disabled = false,
}: SqlEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const initialValueRef = useRef(value);
  const viewRef = useRef<EditorView | null>(null);
  const documentVersionRef = useRef(0);
  const onChangeRef = useRef(onChange);
  const themeId = useSettingsStore((state) => state.themeId);
  const editorFontFamily = useSettingsStore((state) => state.editorFontFamily);
  const editorFontSize = useSettingsStore((state) => state.editorFontSize);
  const themeSettingsRef = useRef({
    themeId,
    editorFontFamily,
    editorFontSize,
  });
  onChangeRef.current = onChange;
  const onSelectionChangeRef = useRef(onSelectionChange);
  onSelectionChangeRef.current = onSelectionChange;
  initialValueRef.current = value;
  themeSettingsRef.current = { themeId, editorFontFamily, editorFontSize };

  useEffect(() => {
    if (!containerRef.current) return undefined;
    const runtime = createSqlIntelligenceRuntime(catalog);
    runtime?.openDocument(
      documentId,
      documentVersionRef.current,
      initialValueRef.current,
      catalog.snapshot.dialect,
    );
    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        documentVersionRef.current += 1;
        const changes: {
          from: number;
          to: number;
          insert: string;
        }[] = [];
        update.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
          changes.push({ from: fromA, to: toA, insert: inserted.toString() });
        });
        runtime?.applyDocumentChanges(
          documentId,
          documentVersionRef.current - 1,
          documentVersionRef.current,
          changes,
        );
        onChangeRef.current(update.state.doc.toString());
      }
      if (update.docChanged || update.selectionSet) {
        onSelectionChangeRef.current?.({
          from: update.state.selection.main.from,
          to: update.state.selection.main.to,
        });
      }
    });
    const platform = getPlatform();
    const state = EditorState.create({
      doc: initialValueRef.current,
      extensions: [
        lineNumbers(),
        EditorView.editable.of(!disabled),
        history(),
        drawSelection(),
        highlightActiveLine(),
        EditorView.domEventHandlers({
          keydown(event, view) {
            const usesNativeModifier =
              platform === "macos" ? event.metaKey : event.ctrlKey;
            if (
              !usesNativeModifier ||
              event.altKey ||
              event.key.toLowerCase() !== "z"
            ) {
              return false;
            }
            const command = event.shiftKey ? redo : undo;
            if (!command(view)) return false;
            event.preventDefault();
            return true;
          },
        }),
        themeCompartment.of(
          createCodeMirrorThemeForId(
            themeSettingsRef.current.themeId,
            themeSettingsRef.current.editorFontFamily,
            themeSettingsRef.current.editorFontSize,
          ),
        ),
        keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
        sql({ dialect: codeMirrorDialect(catalog.snapshot.dialect) }),
        autocompletion({
          activateOnTyping: true,
          override: [
            createSqlCompletionSource(
              catalog,
              documentId,
              () => documentVersionRef.current,
              executionContext,
              runtime,
            ),
          ],
        }),
        updateListener,
      ],
    });
    const view = new EditorView({ state, parent: containerRef.current });
    viewRef.current = view;
    return () => {
      runtime?.closeDocument(documentId);
      viewRef.current = null;
      view.destroy();
    };
  }, [catalog, disabled, documentId, executionContext]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: themeCompartment.reconfigure(
        createCodeMirrorThemeForId(themeId, editorFontFamily, editorFontSize),
      ),
    });
  }, [editorFontFamily, editorFontSize, themeId]);

  useEffect(() => {
    const view = viewRef.current;
    if (view && view.state.doc.toString() !== value) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: value },
      });
    }
  }, [value]);

  return <div ref={containerRef} className={className ?? "min-h-64"} />;
}
