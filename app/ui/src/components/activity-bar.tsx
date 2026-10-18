import { Action } from "./controls";

const views = [
  { id: "explorer", label: "Explorer", icon: "files" },
  { id: "outline", label: "Outline", icon: "list-tree" },
  { id: "source-control", label: "Source Control", icon: "source-control" },
  { id: "scratchpad", label: "Scratchpad", icon: "edit" },
  { id: "todos", label: "Todos", icon: "checklist" },
];

/** VS Code-style activity bar: one icon per sidebar view, then global actions. */
export function ActivityBar(props: {
  view: string;
  sidebar: boolean;
  onShow: (view: string) => void;
  onPalette: () => void;
  onTheme: () => void;
}) {
  return (
    <nav class="activity-bar" aria-label="Activity">
      {views.map((view) => (
        <Action
          label={view.label}
          icon={view.icon}
          active={props.sidebar && props.view === view.id}
          onClick={() => props.onShow(view.id)}
        />
      ))}
      <div class="activity-spacer" />
      <Action
        label="Command Palette"
        icon="terminal"
        onClick={props.onPalette}
      />
      <Action
        label="Toggle Light / Dark"
        icon="color-mode"
        onClick={props.onTheme}
      />
    </nav>
  );
}
