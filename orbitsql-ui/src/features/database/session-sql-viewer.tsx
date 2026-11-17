import { sql } from "@codemirror/lang-sql";
import { EditorState } from "@codemirror/state";
import { EditorView, lineNumbers } from "@codemirror/view";
import { useEffect, useRef } from "react";
import {
  createCodeMirrorThemeForId,
  themeCompartment,
} from "../../design-system";
import { useSettingsStore } from "../../stores/settings.store";
import { codeMirrorDialect } from "../editor/codemirror/sql-language";

export function SessionSqlViewer({
  value,
  className,
}: {
  value: string;
  className?: string;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const valueRef = useRef(value);
  valueRef.current = value;
  const themeId = useSettingsStore((state) => state.themeId);
  const editorFontFamily = useSettingsStore((state) => state.editorFontFamily);
  const editorFontSize = useSettingsStore((state) => state.editorFontSize);

  useEffect(() => {
    const parent = containerRef.current;
    if (!parent) return;
    const state = EditorState.create({
      doc: valueRef.current,
      extensions: [
        lineNumbers(),
        EditorState.readOnly.of(true),
        EditorView.editable.of(false),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ "aria-label": "SQL session text" }),
        themeCompartment.of(
          createCodeMirrorThemeForId(themeId, editorFontFamily, editorFontSize),
        ),
        sql({ dialect: codeMirrorDialect("sqlserver") }),
      ],
    });
    const view = new EditorView({ state, parent });
    viewRef.current = view;
    return () => {
      viewRef.current = null;
      view.destroy();
    };
  }, [editorFontFamily, editorFontSize, themeId]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const currentValue = view.state.doc.toString();
    if (currentValue !== value) {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: value },
      });
    }
  }, [value]);

  return (
    <div
      className={`overflow-hidden rounded-md border border-outline-variant bg-surface-subtle text-label-sm [&_.cm-editor]:max-h-64 [&_.cm-editor]:overflow-auto [&_.cm-editor]:bg-transparent [&_.cm-gutters]:border-r-0 [&_.cm-gutters]:bg-transparent [&_.cm-gutters]:text-on-surface-variant ${className ?? ""}`}
      ref={containerRef}
    />
  );
}
