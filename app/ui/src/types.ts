export type TemplateId =
  "screenplay" | "screenplay-multicam" | "stageplay" | "manuscript";
export interface Story {
  id: number;
  path: string | null;
  text: string;
  revision: number;
  template: TemplateId;
  dirty: boolean;
}
export interface Source {
  file: number;
  bytes: { start: number; end: number };
}
export interface Element {
  kind: string;
  text: string;
  source: Source;
  spans: {
    range: { start: number; end: number };
    style: Record<string, boolean>;
  }[];
  mapping: { display: { start: number; end: number }; source: Source }[];
}
export interface Geometry {
  x: number;
  width: number;
  align: "left" | "center" | "right";
}
export interface Analysis {
  dirty: boolean;
  revision: number;
  elements: Element[];
  outline: { text: string; kind: string; source: Source }[];
  diagnostics: { severity: string; message: string; source: Source }[];
  words: number;
  scenes: number;
  geometry: {
    action: Geometry;
    scene: Geometry;
    cue: Geometry;
    parenthetical: Geometry;
    dialogue: Geometry;
    lyric: Geometry;
  };
}
export interface Preview {
  token: number;
  revision: number;
  pages: number;
}
export interface FileEntry {
  name: string;
  path: string;
}
export const templates: TemplateId[] = [
  "screenplay",
  "screenplay-multicam",
  "stageplay",
  "manuscript",
];
export const templateNames: Record<TemplateId, string> = {
  screenplay: "Screenplay",
  "screenplay-multicam": "Multi-cam",
  stageplay: "Stage Play",
  manuscript: "Manuscript",
};
export const nameOf = (doc: Story) =>
  doc.path?.split("/").pop() ?? "Untitled.md";
