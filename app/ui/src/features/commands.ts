import type { Command } from "../components/controls";

/** Actions the palette and menus can trigger. */
export interface WorkbenchActions {
  run: (job: () => unknown) => () => void;
  showView: (view: string) => void;
  newProject: () => unknown;
  openProject: () => unknown;
  closeProject: () => unknown;
  newFile: () => unknown;
  open: () => unknown;
  save: () => unknown;
  saveAs: () => unknown;
  closeTab: () => unknown;
  exportPdf: () => unknown;
  togglePreview: () => void;
  toggleSidebar: () => void;
  toggleTheme: () => void;
  find: () => void;
}

/** The command palette's list. Ids match native menu items and `shortcut`. */
export function workbenchCommands(a: WorkbenchActions): Command[] {
  const view = (id: string, label: string): Command => ({
    id,
    label,
    run: () => a.showView(id),
  });
  return [
    view("source-control", "View: Source Control"),
    view("scratchpad", "View: Scratchpad"),
    {
      id: "new-project",
      label: "Project: New Project…",
      key: "⇧⌘N",
      run: a.run(a.newProject),
    },
    {
      id: "close-project",
      label: "Project: Close Project",
      key: "⇧⌘W",
      run: a.run(a.closeProject),
    },
    { id: "new", label: "File: New File", key: "⌘N", run: a.run(a.newFile) },
    { id: "open", label: "File: Open…", key: "⌘O", run: a.run(a.open) },
    {
      id: "folder",
      label: "Project: Open Project…",
      key: "⇧⌘O",
      run: a.run(a.openProject),
    },
    { id: "save", label: "File: Save", key: "⌘S", run: a.run(a.save) },
    {
      id: "save-as",
      label: "File: Save As…",
      key: "⇧⌘S",
      run: a.run(a.saveAs),
    },
    {
      id: "close",
      label: "File: Close Tab",
      key: "⌘W",
      run: a.run(a.closeTab),
    },
    {
      id: "export",
      label: "File: Export PDF…",
      key: "⌥⌘E",
      run: a.run(a.exportPdf),
    },
    {
      id: "preview",
      label: "View: Toggle Page Preview",
      key: "⇧⌘V",
      run: a.togglePreview,
    },
    {
      id: "sidebar",
      label: "View: Toggle Sidebar",
      key: "⌘B",
      run: a.toggleSidebar,
    },
    { id: "theme", label: "View: Toggle Light / Dark", run: a.toggleTheme },
    { id: "find", label: "Edit: Find", key: "⌘F", run: a.find },
    view("todos", "View: Todos"),
    view("outline", "View: Outline"),
  ];
}

/** Command id for a ⌘ shortcut handled in the page (menus cover the rest). */
export function shortcut(e: KeyboardEvent): string {
  if (!(e.metaKey || e.ctrlKey)) return "";
  const k = e.key.toLowerCase();
  if (k === "n") return e.shiftKey ? "new-project" : "new";
  if (k === "o") return e.shiftKey ? "folder" : "open";
  if (k === "s") return e.shiftKey ? "save-as" : "save";
  if (k === "w") return e.shiftKey ? "close-project" : "close";
  if (k === "p" && e.shiftKey) return "palette";
  if (k === "v" && e.shiftKey) return "preview";
  if (k === "b") return "sidebar";
  return "";
}
