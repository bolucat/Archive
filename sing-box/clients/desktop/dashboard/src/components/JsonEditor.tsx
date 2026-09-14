import { useEffect, useImperativeHandle, useRef, type Ref } from "react";

import {
  autocompletion,
  closeBrackets,
  closeBracketsKeymap,
  completionStatus,
  startCompletion,
} from "@codemirror/autocomplete";
import {
  defaultKeymap,
  history,
  historyKeymap,
  redo,
  redoDepth,
  undo,
  undoDepth,
} from "@codemirror/commands";
import {
  bracketMatching,
  HighlightStyle,
  LanguageSupport,
  syntaxHighlighting,
} from "@codemirror/language";
import { search, searchKeymap } from "@codemirror/search";
// oxlint-disable-next-line react-doctor/prefer-dynamic-import -- JsonEditor is itself loaded through React.lazy.
import { EditorState } from "@codemirror/state";
// oxlint-disable-next-line react-doctor/prefer-dynamic-import -- JsonEditor is itself loaded through React.lazy.
import { EditorView, keymap, lineNumbers } from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { json5Language } from "codemirror-json5";
import { useLatestRef } from "../app/useLatest";
import {
  jsonSchemaCompletionSource,
  jsonTypingAssist,
  shouldAutoOpenCompletion,
  type JsonSchemaRoot,
} from "./jsonSchemaCompletion";

export type JsonEditorSchema = JsonSchemaRoot;

export interface JsonEditorHandle {
  undo(): void;
  redo(): void;
  insertSymbol(text: string): void;
  replaceAll(text: string): void;
  isCompletionActive(): boolean;
}

const editorTheme = EditorView.theme({
  "&": {
    height: "100%",
    fontSize: "var(--text-md)",
    color: "var(--text)",
    backgroundColor: "var(--surface)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius-md)",
  },
  "&.cm-focused": {
    outline: "none",
  },
  ".cm-scroller": {
    fontFamily: "var(--font-mono)",
    lineHeight: "1.3",
    borderRadius: "var(--radius-md)",
  },
  ".cm-content": {
    caretColor: "var(--text)",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "var(--text)",
  },
  ".cm-gutters": {
    color: "var(--text-faint)",
    backgroundColor: "var(--surface)",
    border: "none",
    borderStartStartRadius: "var(--radius-md)",
    borderEndStartRadius: "var(--radius-md)",
  },
  ".cm-matchingBracket": {
    backgroundColor: "var(--highlight)",
  },
  ".cm-searchMatch": {
    backgroundColor: "var(--medium-soft)",
    outline: "1px solid var(--medium)",
  },
  ".cm-searchMatch-selected": {
    backgroundColor: "var(--accent-soft)",
  },
  ".cm-panels": {
    color: "var(--text)",
    backgroundColor: "var(--card)",
  },
  ".cm-panels-top": {
    borderBottom: "1px solid var(--border)",
  },
  ".cm-panel.cm-search": {
    fontFamily: "var(--font-sans)",
    fontSize: "var(--text-sm)",
  },
  ".cm-textfield": {
    color: "var(--text)",
    backgroundColor: "var(--surface)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius-xs)",
  },
  ".cm-button": {
    color: "var(--text)",
    backgroundImage: "none",
    backgroundColor: "var(--hover)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius-xs)",
  },
  ".cm-tooltip": {
    color: "var(--text)",
    backgroundColor: "var(--card)",
    border: "1px solid var(--border)",
    borderRadius: "var(--radius-xs)",
    maxWidth: "40em",
  },
  ".cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": {
    color: "var(--text)",
    backgroundColor: "var(--accent-soft)",
  },
  ".cm-tooltip .cm-completionMatchedText": {
    color: "var(--info)",
    textDecoration: "none",
  },
});

const jsonHighlight = HighlightStyle.define([
  { tag: tags.propertyName, color: "var(--info)" },
  { tag: tags.string, color: "var(--good)" },
  { tag: tags.number, color: "var(--medium)" },
  { tag: [tags.bool, tags.null], color: "var(--bad)" },
  { tag: tags.comment, color: "var(--text-faint)" },
  { tag: tags.invalid, color: "var(--danger)" },
]);

export function JsonEditor(props: {
  ref?: Ref<JsonEditorHandle>;
  className?: string;
  initialValue: string;
  readOnly?: boolean;
  schema?: JsonEditorSchema | null;
  onChange?: (value: string, canUndo: boolean, canRedo: boolean) => void;
  onSave?: () => void;
  onCompletionOpenChange?: (open: boolean) => void;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const initialValueRef = useRef(props.initialValue);
  const schemaRef = useLatestRef(props.schema);
  const onChangeRef = useLatestRef(props.onChange);
  const onSaveRef = useLatestRef(props.onSave);
  const onCompletionOpenChangeRef = useLatestRef(props.onCompletionOpenChange);

  useEffect(() => {
    let completionOpen = false;
    const completionSource = jsonSchemaCompletionSource(() => schemaRef.current ?? null);
    const view = new EditorView({
      parent: containerRef.current ?? undefined,
      state: EditorState.create({
        doc: initialValueRef.current,
        extensions: [
          lineNumbers(),
          history(),
          bracketMatching(),
          json5Language.data.of({ closeBrackets: { brackets: ["[", "{", '"'] } }),
          new LanguageSupport(json5Language),
          syntaxHighlighting(jsonHighlight),
          autocompletion(),
          closeBrackets(),
          jsonTypingAssist,
          json5Language.data.of({
            autocomplete: completionSource,
          }),
          search({ top: true }),
          keymap.of([
            {
              key: "Mod-s",
              run: () => {
                onSaveRef.current?.();
                return true;
              },
            },
            ...closeBracketsKeymap,
            ...defaultKeymap,
            ...historyKeymap,
            ...searchKeymap,
          ]),
          editorTheme,
          props.readOnly ? [EditorState.readOnly.of(true), EditorView.editable.of(false)] : [],
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              onChangeRef.current?.(
                update.state.doc.toString(),
                undoDepth(update.state) > 0,
                redoDepth(update.state) > 0,
              );
            }
            const open = completionStatus(update.state) === "active";
            if (open !== completionOpen) {
              completionOpen = open;
              onCompletionOpenChangeRef.current?.(open);
            }
            if (
              update.docChanged &&
              update.transactions.some((tr) => shouldAutoOpenCompletion(tr, completionSource))
            ) {
              queueMicrotask(() => {
                if (viewRef.current === view) {
                  startCompletion(view);
                }
              });
            }
          }),
        ],
      }),
    });
    viewRef.current = view;
    return () => {
      viewRef.current = null;
      view.destroy();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useImperativeHandle(
    props.ref,
    () => ({
      undo: () => {
        const view = viewRef.current;
        if (view !== null) {
          undo(view);
          view.focus();
        }
      },
      redo: () => {
        const view = viewRef.current;
        if (view !== null) {
          redo(view);
          view.focus();
        }
      },
      insertSymbol: (text: string) => {
        const view = viewRef.current;
        if (view !== null) {
          view.dispatch(view.state.replaceSelection(text));
          view.focus();
        }
      },
      replaceAll: (text: string) => {
        const view = viewRef.current;
        if (view !== null) {
          view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
        }
      },
      isCompletionActive: () => {
        const view = viewRef.current;
        return view !== null && completionStatus(view.state) === "active";
      },
    }),
    [],
  );

  return <div ref={containerRef} className={props.className} />;
}
