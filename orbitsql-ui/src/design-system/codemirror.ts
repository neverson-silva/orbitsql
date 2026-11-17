import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { Compartment, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { resolveTheme } from "./engine";
import type { Theme } from "./types";

export const themeCompartment = new Compartment();

export function createCodeMirrorTheme(
  theme: Theme,
  fontFamily: string,
  fontSize: number,
): Extension {
  const { colors } = theme;
  const { syntax } = colors;
  const codeLineHeight =
    Math.round(
      theme.typography["code-md"].lineHeight *
        (fontSize / theme.typography["code-md"].fontSize) *
        100,
    ) / 100;

  const editorTheme = EditorView.theme(
    {
      "&": {
        backgroundColor: colors.surfaceContainerLowest,
        color: colors.onSurface,
        height: "100%",
      },
      ".cm-content": {
        caretColor: colors.primary,
        fontFamily,
        fontSize: `${fontSize}px`,
        lineHeight: `${codeLineHeight}px`,
        padding: "12px 0 12px 10px",
      },
      ".cm-cursor, .cm-dropCursor": {
        borderLeftColor: colors.primary,
        borderLeftWidth: "2px",
      },
      "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
        {
          backgroundColor: `color-mix(in srgb, ${colors.primary} 22%, transparent)`,
        },
      ".cm-activeLine": {
        backgroundColor: "transparent",
      },
      ".cm-gutters": {
        backgroundColor: colors.surfaceContainerLowest,
        color: colors.onSurfaceVariant,
        border: "none",
        borderRight: `1px solid ${colors.outline}`,
        paddingRight: "0",
      },
      ".cm-activeLineGutter": {
        backgroundColor: "transparent",
      },
      ".cm-lineNumbers .cm-gutterElement": {
        padding: "0 8px 0 12px",
        minWidth: "32px",
        textAlign: "right",
      },
      ".cm-foldGutter .cm-gutterElement": {
        padding: "0 4px",
      },
      ".cm-tooltip": {
        backgroundColor: colors.surfaceContainerHigh,
        border: `1px solid ${colors.outlineVariant}`,
        borderRadius: theme.radii.md,
        color: colors.onSurface,
      },
      ".cm-tooltip.cm-tooltip-autocomplete": {
        backgroundColor: colors.surfaceContainerHighest,
        border: `1px solid ${colors.outline}`,
        borderRadius: theme.radii.md,
        boxShadow: theme.elevation.overlay,
        overflow: "hidden",
        zIndex: "20",
        "& > ul > li": {
          padding: "4px 8px",
        },
        "& > ul > li[aria-selected]": {
          backgroundColor: `color-mix(in srgb, ${colors.primary} 18%, transparent)`,
          color: colors.onSurface,
        },
      },
      ".cm-completionIcon": {
        marginRight: "8px",
      },
      ".cm-completionLabel": {
        fontFamily,
      },
      ".cm-completionDetail": {
        marginLeft: "8px",
        fontStyle: "italic",
        color: colors.onSurfaceVariant,
      },
      ".cm-panels": {
        backgroundColor: colors.surfaceContainer,
        color: colors.onSurface,
      },
      ".cm-searchMatch": {
        backgroundColor: `color-mix(in srgb, ${colors.warning} 25%, transparent)`,
      },
      ".cm-searchMatch.cm-searchMatch-selected": {
        backgroundColor: `color-mix(in srgb, ${colors.primary} 28%, transparent)`,
      },
    },
    { dark: theme.type === "dark" },
  );

  const highlightStyle = HighlightStyle.define([
    { tag: tags.keyword, color: syntax.keyword },
    { tag: tags.operator, color: syntax.operator },
    { tag: tags.string, color: syntax.string },
    { tag: tags.number, color: syntax.number },
    { tag: tags.comment, color: syntax.comment, fontStyle: "italic" },
    { tag: tags.function(tags.variableName), color: syntax.function },
    { tag: tags.variableName, color: syntax.variable },
    { tag: tags.name, color: syntax.table },
    { tag: tags.typeName, color: syntax.type },
    { tag: tags.propertyName, color: syntax.variable },
    { tag: tags.className, color: syntax.table },
    { tag: tags.bracket, color: syntax.operator },
    { tag: tags.punctuation, color: syntax.operator },
    { tag: tags.definitionKeyword, color: syntax.keyword },
    { tag: tags.standard(tags.name), color: syntax.function },
  ]);

  return [editorTheme, syntaxHighlighting(highlightStyle)];
}

export function createCodeMirrorThemeForId(
  themeId: string,
  fontFamily: string,
  fontSize: number,
): Extension {
  return createCodeMirrorTheme(resolveTheme(themeId), fontFamily, fontSize);
}
