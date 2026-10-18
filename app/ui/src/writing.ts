import {
  EditorState,
  StateEffect,
  StateField,
  type Extension,
} from "@codemirror/state";
import { keymap } from "@codemirror/view";
import {
  autocompletion,
  acceptCompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";
import type { Analysis } from "./types";

type Catalog = {
  enabled: boolean;
  characters: string[];
  locations: string[];
  times: string[];
};
export const writingContext = StateEffect.define<{
  enabled: boolean;
  analysis: Analysis;
}>();
const prefixes = [
  "INT.",
  "EXT.",
  "EST.",
  "INT./EXT.",
  "INT/EXT.",
  "I./E.",
  "E./I.",
];
const standardTimes = [
  "DAY",
  "NIGHT",
  "DAWN",
  "DUSK",
  "MORNING",
  "AFTERNOON",
  "EVENING",
  "LATER",
  "CONTINUOUS",
  "SAME TIME",
];
const scenePrefix =
  /^(?:INT\.\/EXT\.|INT\/EXT\.|INT\.|EXT\.|EST\.|I\.\/E\.|E\.\/I\.)\s*/i;
function unique(items: string[]) {
  return [...new Set(items.filter(Boolean))].sort();
}
const catalog = StateField.define<Catalog>({
  create: () => ({ enabled: false, characters: [], locations: [], times: [] }),
  update(value, tr) {
    for (const effect of tr.effects) {
      if (!effect.is(writingContext)) continue;
      const { enabled, analysis } = effect.value;
      const characters: string[] = [],
        locations: string[] = [],
        times: string[] = [];
      const cursorByte = new TextEncoder().encode(
        tr.newDoc.sliceString(0, tr.newSelection.main.head),
      ).length;
      for (const element of analysis.elements) {
        if (element.source.file !== 0) continue;
        // Do not learn a provisional name/location/time from the paragraph
        // currently being typed, then mistake it for an exact known match.
        const range = element.source.bytes;
        if (range && range.start <= cursorByte && cursorByte <= range.end)
          continue;
        if (element.kind === "cue")
          characters.push(
            element.text
              .replace(/\s*(?:\(.*|\^\s*$)/, "")
              .trim()
              .toUpperCase(),
          );
        if (element.kind === "scene") {
          const body = element.text
            .replace(/^\./, "")
            .replace(scenePrefix, "")
            .replace(/\s*#[^#]+#\s*$/, "")
            .trim();
          const divider = [...body.matchAll(/\s+[-–—]\s+/g)].at(-1);
          locations.push(
            (divider ? body.slice(0, divider.index) : body)
              .trim()
              .toUpperCase(),
          );
          if (divider)
            times.push(
              body
                .slice(divider.index! + divider[0].length)
                .trim()
                .toUpperCase(),
            );
        }
      }
      value = {
        enabled,
        characters: unique(characters),
        locations: unique(locations),
        times: unique([...standardTimes, ...times]),
      };
    }
    return value;
  },
});

/** Mirror the parser's non-nesting notes/boneyard boundaries, including escaped
 * openers. A marker inside one comment does not open another kind of comment. */
function inComment(text: string) {
  let closing: string | null = null;
  for (const match of text.matchAll(/\/\*|\*\/|\[\[|\]\]/g)) {
    if (closing) {
      if (match[0] === closing) closing = null;
      continue;
    }
    if (match[0] !== "/*" && match[0] !== "[[") continue;
    let slashes = 0;
    for (let i = match.index! - 1; i >= 0 && text[i] === "\\"; i--) slashes++;
    if (slashes % 2 === 0) closing = match[0] === "/*" ? "*/" : "]]";
  }
  return closing !== null;
}
/** Automatic assistance is limited to paragraph boundaries outside comments. */
function paragraph(state: EditorState, pos: number) {
  const line = state.doc.lineAt(pos);
  if (line.number > 1 && state.doc.line(line.number - 1).text.trim())
    return null;
  const before = state.doc.sliceString(0, line.from);
  if (inComment(before)) return null;
  return line;
}
function suggestions(
  from: number,
  to: number,
  labels: string[],
  detail: string,
  type: string,
  fragment: string,
): CompletionResult | null {
  if (labels.includes(fragment.toUpperCase())) return null;
  labels = labels.filter(
    (label) =>
      label.startsWith(fragment.toUpperCase()) &&
      label !== fragment.toUpperCase(),
  );
  if (!labels.length) return null;
  return {
    from,
    to,
    options: labels.map(
      (label) => ({ label, detail, type }) satisfies Completion,
    ),
  };
}
export function scriptCompletions(
  context: CompletionContext,
): CompletionResult | null {
  const data = context.state.field(catalog);
  if (!data.enabled || context.state.selection.ranges.length !== 1) return null;
  const line = paragraph(context.state, context.pos);
  if (!line) return null;
  const typed = line.text.slice(0, context.pos - line.from);
  const prefix = typed.match(scenePrefix)?.[0];
  const forcedScene =
    typed.startsWith(".") && !typed.startsWith("..") && !typed.startsWith(". ");
  if (prefix || forcedScene) {
    const bodyStart = line.from + (prefix?.length ?? 1);
    const body = typed.slice(bodyStart - line.from);
    const time = [...body.matchAll(/\s+[-–—]\s+/g)].at(-1);
    if (time)
      return suggestions(
        bodyStart + time.index! + time[0].length,
        line.to,
        data.times,
        "Time",
        "keyword",
        body.slice(time.index! + time[0].length),
      );
    const fullBody = line.text.slice(bodyStart - line.from);
    const end = [...fullBody.matchAll(/\s+[-–—]\s+/g)].at(-1)?.index ?? -1;
    return suggestions(
      bodyStart,
      end < 0 ? line.to : bodyStart + end,
      data.locations,
      "Location",
      "text",
      body,
    );
  }
  const forcedCue = typed.startsWith("@");
  const fragment = forcedCue ? typed.slice(1) : typed;
  if (!fragment && !forcedCue && !context.explicit) return null;
  if (!/^[\p{L}\p{M}\d .'-]*$/u.test(fragment)) return null;
  // Lowercase prose should not summon a character picker. @ explicitly requests
  // a character; otherwise use capitals, or invoke completion with Control-Space.
  if (!forcedCue && !context.explicit && fragment !== fragment.toUpperCase())
    return null;
  const from = line.from + (forcedCue ? 1 : 0);
  const suffix = line.text.search(/\s*(?:\(|\^)/);
  const to = suffix < 0 ? line.to : line.from + suffix;
  if (context.pos > to) return null;
  const options: Completion[] = data.characters.map((label) => ({
    label,
    detail: "Character",
    type: "variable",
  }));
  if (data.characters.includes(fragment.toUpperCase())) return null;
  if (!forcedCue)
    options.push(
      ...prefixes.map((label) => ({
        label,
        apply: label + " ",
        detail: "Scene heading",
        type: "keyword",
      })),
    );
  const matching = options.filter(
    (option) =>
      option.label.startsWith(fragment.toUpperCase()) &&
      option.label !== fragment.toUpperCase(),
  );
  return matching.length ? { from, to, options: matching } : null;
}

const capitals = EditorState.transactionFilter.of((tr) => {
  if (
    !tr.docChanged ||
    !tr.isUserEvent("input.type") ||
    tr.isUserEvent("input.type.compose") ||
    !tr.startState.field(catalog).enabled ||
    tr.newSelection.ranges.length !== 1
  )
    return tr;
  const lines = new Set<number>();
  let multiline = false;
  tr.changes.iterChanges((_a, _b, from, to, inserted) => {
    if (inserted.lines > 1) multiline = true;
    lines.add(tr.newDoc.lineAt(from).number);
    lines.add(tr.newDoc.lineAt(to).number);
  });
  if (multiline) return tr;
  const changes: { from: number; to: number; insert: string }[] = [];
  for (const number of [...lines].sort((a, b) => a - b)) {
    const line = paragraph(tr.state, tr.newDoc.line(number).from);
    if (!line) continue;
    const text = line.text;
    const heading = scenePrefix.test(text) || /^\.(?![.\s])/.test(text);
    const cue = text.startsWith("@");
    if (!heading && !cue) continue;
    const end = cue
      ? text.search(/[\(^]/) < 0
        ? text.length
        : text.search(/[\(^]/)
      : text.length;
    let offset = 0;
    for (const char of text.slice(0, end)) {
      const upper = char.toUpperCase();
      if (char !== upper)
        changes.push({
          from: line.from + offset,
          to: line.from + offset + char.length,
          insert: upper,
        });
      offset += char.length;
    }
  }
  return changes.length ? [tr, { changes, sequential: true }] : tr;
});

export function writingSupport(enabled: boolean): Extension {
  return [
    catalog.init(() => ({
      enabled,
      characters: [],
      locations: [],
      times: standardTimes,
    })),
    capitals,
    autocompletion({
      override: [scriptCompletions],
      activateOnTypingDelay: 180,
      icons: false,
      maxRenderedOptions: 12,
      interactionDelay: 0,
    }),
    keymap.of([{ key: "Tab", run: acceptCompletion }]),
  ];
}
