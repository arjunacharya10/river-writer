import assert from "node:assert/strict";
import { EditorState } from "@codemirror/state";
import { CompletionContext } from "@codemirror/autocomplete";
import { history, undo } from "@codemirror/commands";
import {
  writingSupport,
  writingContext,
  scriptCompletions,
} from "./ui/src/writing.ts";
const analysis = {
  elements: [
    { kind: "cue", text: "MAYA (V.O.)", source: { file: 0 } },
    { kind: "cue", text: "JON", source: { file: 0 } },
    { kind: "cue", text: "MAYANA", source: { file: 0 } },
    { kind: "cue", text: "INCLUDED NAME", source: { file: 1 } },
    { kind: "scene", text: "INT. CAFÉ - NIGHT", source: { file: 0 } },
    { kind: "scene", text: "EXT. RIVER - DAY", source: { file: 0 } },
  ],
};
function state(text, enabled = true, pos = text.length) {
  let s = EditorState.create({
    doc: text,
    selection: { anchor: pos },
    extensions: [history(), writingSupport(enabled)],
  });
  return s.update({ effects: writingContext.of({ enabled, analysis }) }).state;
}
function input(s, text, userEvent = "input.type") {
  const range = s.selection.main;
  return s.update({
    changes: { from: range.from, to: range.to, insert: text },
    selection: { anchor: range.from + text.length },
    userEvent,
  }).state;
}
function complete(s, explicit = false) {
  return scriptCompletions(
    new CompletionContext(s, s.selection.main.head, explicit),
  );
}
assert.equal(input(state("int"), ".").doc.toString(), "INT.");
assert.equal(input(state("INT. caf"), "é").doc.toString(), "INT. CAFÉ");
assert.equal(input(state("@ma"), "y").doc.toString(), "@MAY");
let unicode = input(state("@stra"), "ße");
assert.equal(unicode.doc.toString(), "@STRASSE");
assert.equal(unicode.selection.main.head, 8);
const cursor = input(state("INT. CAFE - DAY", true, 7), "é");
assert.equal(cursor.doc.toString(), "INT. CAÉFE - DAY");
assert.equal(cursor.selection.main.head, 8);
for (const text of [
  "ordinary prose",
  "Action paragraph.\nint. room",
  "/*\n\nint. room",
  "[[note\n\n@maya",
]) {
  assert.equal(input(state(text), "x").doc.toString(), text + "x");
}
assert.equal(input(state("int", false), ".").doc.toString(), "int.");
for (const prefix of [
  String.raw`\[[ literal`,
  "/* [[ hidden */",
  "[[ /* hidden ]]",
]) {
  assert(
    complete(state(prefix + "\n\n@MA")).options.some((o) => o.label === "MAYA"),
  );
}
assert.equal(
  input(state("int."), " room", "input.paste").doc.toString(),
  "int. room",
);
assert.equal(
  input(state("int."), " room", "input.type.compose").doc.toString(),
  "int. room",
);
assert.equal(
  input(state(""), "int. room\n\n@maya").doc.toString(),
  "int. room\n\n@maya",
);
const changed = input(state("@m"), "a");
const undoTarget = {
  state: changed,
  dispatch: (tr) => {
    undoTarget.state = tr.state;
  },
};
assert.equal(undo(undoTarget), true);
assert.equal(undoTarget.state.doc.toString(), "@m");
assert(complete(state("@MA")).options.some((o) => o.label === "MAYA"));
assert.equal(complete(state("@MAYA")), null);
assert.equal(complete(state("INT. CAFÉ - NIGHT")), null);
assert(
  !complete(state("@MA")).options.some((o) => o.label === "INCLUDED NAME"),
);
assert(complete(state("INT. CA")).options.some((o) => o.label === "CAFÉ"));
assert(
  complete(state("INT. CAFÉ - NI")).options.some((o) => o.label === "NIGHT"),
);
assert(complete(state("IN")).options.some((o) => o.label === "INT."));
assert(complete(state(""), true).options.some((o) => o.label === "MAYA"));
for (const text of [
  "ordinary prose",
  "MAYA\nHello",
  "/*\n\n@MA",
  "[[note\n\nINT. CA",
])
  assert.equal(complete(state(text)), null);
assert.equal(complete(state("@MA", false)), null);
// Changing syntax (e.g. Save As .fountain) updates assistance without losing history.
let switched = state("int", false);
switched = switched.update({
  effects: writingContext.of({ enabled: true, analysis }),
}).state;
assert.equal(input(switched, ".").doc.toString(), "INT.");

// A parser update during typing must not learn the incomplete heading as an
// exact match and hide known, longer location/time suggestions.
for (const [partial, expected] of [
  ["INT. CA", "CAFÉ"],
  ["INT. CAFÉ - NI", "NIGHT"],
]) {
  const first = "EXT. CAFÉ - NIGHT";
  const text = first + "\n\n" + partial;
  const encode = (value) => new TextEncoder().encode(value).length;
  let s = state(text);
  s = s.update({
    effects: writingContext.of({
      enabled: true,
      analysis: {
        elements: [
          {
            kind: "scene",
            text: first,
            source: { file: 0, bytes: { start: 0, end: encode(first) } },
          },
          {
            kind: "scene",
            text: partial,
            source: {
              file: 0,
              bytes: { start: encode(first + "\n\n"), end: encode(text) },
            },
          },
        ],
      },
    }),
  }).state;
  assert(complete(s).options.some((option) => option.label === expected));
}
console.log(
  "PASS: screenplay completions, Unicode/cursor mapping, scoped capitals, prose/comment/paste/IME protection, atomic Undo, syntax switching.",
);
