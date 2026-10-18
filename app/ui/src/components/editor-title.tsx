import { For, Show } from "solid-js";
import { Action, Icon, TemplatePicker } from "./controls";
import { nameOf, type Story, type TemplateId } from "../types";

/** Story tabs plus the active story's template picker and pane toggles. */
export function EditorTitle(props: {
  tabs: Story[];
  active: number | null;
  current: Story | undefined;
  onSelect: (id: number) => void;
  onClose: (id: number) => void;
  onTemplate: (template: TemplateId) => void;
  previewOpen: boolean;
  onPreview: () => void;
  onExport: () => void;
}) {
  return (
    <header class="editor-title">
      <div class="tabs" role="tablist" aria-label="Open stories">
        <For each={props.tabs}>
          {(doc) => (
            <div class={`tab ${props.active === doc.id ? "selected" : ""}`}>
              <button
                role="tab"
                aria-selected={props.active === doc.id}
                onClick={() => props.onSelect(doc.id)}
                title={doc.path ?? "Unsaved file"}
              >
                <Icon name="file-text" />
                {nameOf(doc)}
                <span
                  class="dirty"
                  aria-label={doc.dirty ? "Unsaved changes" : "Saved"}
                >
                  {doc.dirty ? "●" : ""}
                </span>
              </button>
              <button
                class="tab-close"
                aria-label={`Close ${nameOf(doc)}`}
                onClick={() => props.onClose(doc.id)}
              >
                <Icon name="close" />
              </button>
            </div>
          )}
        </For>
      </div>
      <Show when={props.current}>
        {(doc) => (
          <div class="editor-actions">
            <TemplatePicker
              value={doc().template}
              onChange={props.onTemplate}
            />
            <Action
              label="Toggle page preview"
              icon="open-preview"
              active={props.previewOpen}
              onClick={props.onPreview}
            />
            <Action label="Export PDF" icon="export" onClick={props.onExport} />
          </div>
        )}
      </Show>
    </header>
  );
}

/** Shown in the editor area when no story tab is open. */
export function EmptyEditor(props: {
  onNew: () => void;
  onOpen: () => void;
  onOpenProject: () => void;
}) {
  return (
    <section class="welcome">
      <h1>River Writer</h1>
      <p>Your story starts with a few words.</p>
      <button onClick={props.onNew}>
        New File <kbd>⌘N</kbd>
      </button>
      <button onClick={props.onOpen}>
        Open File… <kbd>⌘O</kbd>
      </button>
      <button onClick={props.onOpenProject}>
        Open Project… <kbd>⇧⌘O</kbd>
      </button>
    </section>
  );
}
