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
/** Positions are UTF-16 offsets in the editor text, computed by Rust. */
export interface Element {
  kind: string;
  from: number;
  to: number;
  text?: string;
  marks?: { from: number; to: number; style: Record<string, boolean> }[];
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
  outline: { text: string; kind: string; at: number }[];
  diagnostics: { severity: string; message: string; at: number }[];
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

export interface ProjectSession {
  sidebarSizes: [number, number];
  editorSizes: [number, number];
  openFiles: string[];
  activeFile: string | null;
  cursors: Record<string, number>;
  scrolls: Record<string, number>;
  sidebar: boolean;
  sideView: string;
  preview: boolean;
  theme: string | null;
}
export interface Project {
  path: string;
  name: string;
  files: FileEntry[];
  session: ProjectSession;
}
export interface RecentProject {
  path: string;
  name: string;
  lastOpened: number;
  available: boolean;
}
