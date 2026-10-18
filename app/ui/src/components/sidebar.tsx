import { For, Show } from "solid-js";
import { Action, Icon } from "./controls";
import type { FileEntry } from "../types";

const titles: Record<string, string> = {
  "source-control": "SOURCE CONTROL",
  scratchpad: "SCRATCHPAD",
  todos: "TODOS",
  outline: "OUTLINE",
};
export function SidebarTitle(props: {
  view: string;
  onOpenProject: () => void;
}) {
  return (
    <header>
      {titles[props.view] ?? "EXPLORER"}
      <Action
        label="Open Project"
        icon="folder-opened"
        onClick={props.onOpenProject}
      />
    </header>
  );
}

/** Story files in the open project (a flat list for now). */
export function ExplorerView(props: {
  folder: string;
  files: FileEntry[];
  currentPath: string | null | undefined;
  onOpen: (path: string) => void;
  onOpenProject: () => void;
}) {
  return (
    <Show
      when={props.folder}
      fallback={
        <div class="sidebar-welcome">
          <p>Open a folder to explore your story files.</p>
          <button class="primary" onClick={props.onOpenProject}>
            Open Project
          </button>
        </div>
      }
    >
      <div class="folder-title" title={props.folder}>
        {props.folder.split("/").pop()}
      </div>
      <For each={props.files}>
        {(file) => (
          <button
            class={`file ${props.currentPath === file.path ? "selected" : ""}`}
            title={file.path}
            onClick={() => props.onOpen(file.path)}
          >
            <Icon name="file-text" />
            <span>{file.name}</span>
          </button>
        )}
      </For>
      <Show when={!props.files.length}>
        <p class="muted">No .md or .fountain files in this folder.</p>
      </Show>
    </Show>
  );
}

/** Scenes, sections and synopses of the active story; click to jump there. */
export function OutlineView(props: {
  items: { text: string; kind: string; at: number }[] | undefined;
  onJump: (at: number) => void;
}) {
  return (
    <div class="outline">
      <For each={props.items}>
        {(item) => (
          <button onClick={() => props.onJump(item.at)}>
            <Icon
              name={item.kind === "scene" ? "symbol-event" : "symbol-namespace"}
            />
            <span>{item.text}</span>
          </button>
        )}
      </For>
      <Show when={!props.items?.length}>
        <p class="muted">Scenes and headings appear here.</p>
      </Show>
    </div>
  );
}
