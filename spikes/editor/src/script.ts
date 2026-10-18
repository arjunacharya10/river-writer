import {StateField, RangeSetBuilder, EditorState} from "@codemirror/state";
import {Decoration, EditorView, type DecorationSet} from "@codemirror/view";
import template from "../../template.json";

export type Kind = "action" | "scene" | "cue" | "parenthetical" | "dialogue" | "transition" | "note" | "heading";
export function classify(text: string): Kind[] {
  let dialogue = false, boneyard = false;
  const lines = text.split("\n");
  return lines.map((raw, index) => {
    const line = raw.trim();
    if (!line) { dialogue = false; return "action"; }
    if (line.startsWith("/*")) boneyard = true;
    if (boneyard) {if (line.includes("*/")) boneyard = false; return "note";}
    if (/^(\[\[|=)/.test(line)) return "note";
    if (/^#/.test(line)) {dialogue = false; return "heading";}
    if (/^(\.(?!\.)|(?:INT\.?|EXT\.?|INT\.\/EXT\.?|I\/E\.?)(?:\s|\.))/i.test(line)) {dialogue = false; return "scene";}
    if (/^>|TO:$/.test(line)) {dialogue = false; return "transition";}
    if (/^!/.test(line)) {dialogue = false; return "action";}
    if (line.startsWith("@") || (!dialogue && (index === 0 || !lines[index-1].trim()) && /[A-Z]/.test(line) && line === line.toUpperCase() && !!lines[index+1]?.trim())) {
      dialogue = true; return "cue";
    }
    if (dialogue) return line.startsWith("(") ? "parenthetical" : "dialogue";
    return "action";
  });
}
export function scriptLook(manuscript = false) {
  const build = (state: EditorState) => {
    const kinds = manuscript ? [] : classify(state.doc.toString());
    const builder = new RangeSetBuilder<Decoration>();
    for (let n=1; n<=state.doc.lines; n++) {
      const line = state.doc.line(n);
      const kind = manuscript ? (line.text.startsWith("#") ? "heading" : "action") : kinds[n-1];
      const geometry = template.screenplay[kind as keyof typeof template.screenplay] ?? template.screenplay.action;
      const left = manuscript ? 0 : (geometry.x-template.screenplay.action.x)/template.cellPt;
      const width = manuscript ? 72 : geometry.width/template.cellPt;
      builder.add(line.from,line.from,Decoration.line({attributes:{
        class:`script-${kind}`, "data-kind":kind,
        style:`margin-left:${left}ch;width:${width}ch;max-width:calc(100% - ${left}ch);`
      }}));
      if (!manuscript && /^[@.!>~]/.test(line.text)) {
        builder.add(line.from,line.from+1,Decoration.mark({class:"forcing-marker"}));
      }
    }
    return builder.finish();
  };
  return StateField.define<DecorationSet>({create:build,update:(value,tr)=>tr.docChanged ? build(tr.state) : value,provide:f=>EditorView.decorations.from(f)});
}
export const shortScript = `EXT. FERRY TERMINAL - NIGHT

Rain taps the roof of a deserted ferry terminal. MAYA sets a brass key on the counter. Across the river, a single window flickers.

MAYA
(turning the key)
The last boat left an hour ago. If someone is still over there, they found another way across.

A bell sounds once. Maya looks toward the water.

ELIAS (O.S.)
Then we wait for the light.

CUT TO:

INT. SIGNAL ROOM - NIGHT

An empty chair. A wet coat. The radio begins to hiss.

[[Check the sound of the radio.]]

`;
export const prose = `# The Other Bank

By morning the river had carried away the steps. Nora stood where the garden ended and watched a branch turn slowly in the current. She had expected the house to feel smaller after everyone left. Instead, each empty room seemed to have acquired another door.

She put the key in her pocket. On the far bank, the ferryman raised one hand, as though they had arranged to meet. Nora did not wave back.
`;
export function longScript() {
  return Array.from({length:120},(_,i)=>`EXT. FERRY TERMINAL ${i+1} - NIGHT\n\n` + shortScript.split("\n").slice(2).join("\n").repeat(2)).join("\n");
}
