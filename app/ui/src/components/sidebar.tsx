import { For, Show, createSignal } from "solid-js";
import { Action, Icon } from "./controls";
import type { FileEntry } from "../types";

const titles: Record<string, string> = {
  "source-control": "SOURCE CONTROL",
  scratchpad: "SCRATCHPAD",
  todos: "TODOS",
  outline: "OUTLINE",
};
/** Open Project lives in the File menu (⇧⌘O), not here. */
export function SidebarTitle(props: {
  view: string;
  /** Present when the Explorer can create a file in the open project. */
  onNewFile?: () => void;
}) {
  return (
    <header>
      {titles[props.view] ?? "EXPLORER"}
      <Show when={props.onNewFile}>
        {(onNewFile) => (
          <Action label="New File…" icon="new-file" onClick={onNewFile()} />
        )}
      </Show>
    </header>
  );
}

/** VS Code-style inline name field for a new file at the top of the Explorer. */
function NewFileField(props: {
  onCreate: (name: string) => Promise<void>;
  onCancel: () => void;
}) {
  const [name, setName] = createSignal("");
  const [error, setError] = createSignal("");
  let busy = false;
  async function submit() {
    if (busy) return;
    if (!name().trim()) return props.onCancel();
    busy = true;
    try {
      await props.onCreate(name());
    } catch (e) {
      setError(String(e).replace(/^Error: /, ""));
    } finally {
      busy = false;
    }
  }
  return (
    <div class="new-file">
      <div class="file">
        <Icon name="file-text" />
        <input
          ref={(el) => queueMicrotask(() => el.focus())}
          aria-label="New file name"
          placeholder="Name, e.g. Act One or Pilot.fountain"
          value={name()}
          onInput={(e) => {
            setName(e.currentTarget.value);
            setError("");
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void submit();
            } else if (e.key === "Escape") {
              e.preventDefault();
              props.onCancel();
            }
          }}
          onBlur={() => {
            // Like VS Code: clicking away creates the file, or cancels after an error.
            if (error()) props.onCancel();
            else void submit();
          }}
        />
      </div>
      <Show when={error()}>
        <p class="new-file-error" role="alert">
          {error()}
        </p>
      </Show>
    </div>
  );
}

/** Story files in the open project (a flat list for now). */
export function ExplorerView(props: {
  folder: string;
  files: FileEntry[];
  currentPath: string | null | undefined;
  onOpen: (path: string) => void;
  onOpenProject: () => void;
  naming: boolean;
  onCreate: (name: string) => Promise<void>;
  onCancelCreate: () => void;
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
      <Show when={props.naming}>
        <NewFileField
          onCreate={props.onCreate}
          onCancel={props.onCancelCreate}
        />
      </Show>
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
      <Show when={!props.files.length && !props.naming}>
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
