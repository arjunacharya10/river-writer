import {
  EditorState,
  StateEffect,
  StateField,
  type Extension,
} from "@codemirror/state";
import {
  EditorView,
  Decoration,
  keymap,
  drawSelection,
  highlightActiveLine,
  type DecorationSet,
} from "@codemirror/view";
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
} from "@codemirror/commands";
import {
  search,
  searchKeymap,
  highlightSelectionMatches,
} from "@codemirror/search";
import { showMinimap } from "@replit/codemirror-minimap";
import type { Analysis, Story } from "./types";
import { writingSupport } from "./writing";
export const semantic = StateEffect.define<{
  analysis: Analysis;
  script: boolean;
}>();
const decorations = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(value, tr) {
    value = value.map(tr.changes);
    for (const effect of tr.effects) {
      if (!effect.is(semantic)) continue;
      const { analysis, script } = effect.value;
      const ranges: ReturnType<Decoration["range"]>[] = [];
      const used = new Set<number>();
      for (const el of analysis.elements) {
        const length = tr.state.doc.length;
        const start = Math.min(el.from, length),
          end = Math.min(el.to, length);
        const g = analysis.geometry;
        let geometry = g.action;
        if (
          ["scene", "cue", "parenthetical", "dialogue", "lyric"].includes(
            el.kind,
          )
        )
          geometry = g[el.kind as keyof typeof g];
        const left = script
          ? Math.max(0, (geometry.x - g.action.x) / 7.1953125)
          : 0;
        const width = script ? geometry.width / 7.1953125 : 72;
        for (let pos = start; pos <= end;) {
          const line = tr.state.doc.lineAt(pos);
          if (!used.has(line.from)) {
            used.add(line.from);
            ranges.push(
              Decoration.line({
                attributes: {
                  class: `story-${el.kind}`,
                  style: `margin-left:${left}ch;width:${width}ch;max-width:calc(100% - ${left}ch);text-align:${el.kind === "transition" ? "right" : el.kind === "centered" ? "center" : "left"};`,
                },
              }).range(line.from),
            );
          }
          if (line.to >= end) break;
          pos = line.to + 1;
        }
        for (const mark of el.marks ?? []) {
          const classes = Object.entries(mark.style)
            .filter(([, v]) => v)
            .map(([key]) => `text-${key}`)
            .join(" ");
          const to = Math.min(mark.to, tr.state.doc.length);
          if (classes && mark.from < to)
            ranges.push(
              Decoration.mark({ class: classes }).range(mark.from, to),
            );
        }
      }
      value = Decoration.set(ranges, true);
    }
    return value;
  },
  provide: (f) => EditorView.decorations.from(f),
});
export function stateFor(
  doc: Story,
  changed: (text: string) => void,
): EditorState {
  return EditorState.create({
    doc: doc.text,
    extensions: [
      history(),
      drawSelection(),
      highlightActiveLine(),
      search({ top: true }),
      highlightSelectionMatches(),
      EditorView.lineWrapping,
      decorations,
      writingSupport(doc.path?.toLowerCase().endsWith(".fountain") ?? false),
      keymap.of([
        ...defaultKeymap,
        ...historyKeymap,
        ...searchKeymap,
        indentWithTab,
      ]),
      showMinimap.compute(["doc"], () => ({
        create: () => ({ dom: document.createElement("div") }),
        displayText: "blocks",
        showOverlay: "mouse-over",
      })),
      EditorView.contentAttributes.of({
        spellcheck: "true",
        "aria-label": "Story editor",
        autocapitalize: "sentences",
      }),
      EditorView.updateListener.of((v) => {
        if (v.docChanged) changed(v.state.doc.toString());
      }),
      EditorView.theme({
        "&": {
          height: "100%",
          backgroundColor: "var(--editor)",
          color: "var(--text)",
        },
        ".cm-scroller": {
          overflow: "auto",
          fontFamily: '"Courier Prime",monospace',
          fontSize: "16px",
          lineHeight: "1.55",
        },
        ".cm-content": { caretColor: "var(--text)" },
        ".cm-cursor": { borderLeftColor: "var(--text)" },
        ".cm-activeLine": { backgroundColor: "var(--active-line)" },
        "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection":
          { background: "var(--selection)" },
        ".cm-panels": {
          backgroundColor: "var(--sidebar)",
          color: "var(--text)",
        },
        ".cm-search input": {
          color: "var(--text)",
          background: "var(--editor)",
        },
        ".cm-button": { color: "var(--text)", background: "var(--hover)" },
      }) as Extension,
    ],
  });
}
