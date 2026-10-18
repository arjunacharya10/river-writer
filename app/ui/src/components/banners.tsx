import { For, Show } from "solid-js";
import { Action } from "./controls";

/** Offered at start-up when an interrupted session left unsaved writing. */
export function RecoveryNotice(props: {
  drafts: { key: string; path: string | null }[];
  disabled: boolean;
  onRecover: (key: string) => void;
}) {
  return (
    <Show when={props.drafts.length > 0}>
      <aside class="recovery-notice" aria-label="Recovered drafts">
        <span>River found writing from an interrupted session.</span>
        <For each={props.drafts}>
          {(draft) => (
            <button
              disabled={props.disabled}
              onClick={() => props.onRecover(draft.key)}
            >
              Recover {draft.path?.split("/").at(-1) ?? "Untitled"}
            </button>
          )}
        </For>
      </aside>
    </Show>
  );
}

/** Errors stay until dismissed; they usually need the writer's attention. */
export function ErrorBanner(props: { error: string; onDismiss: () => void }) {
  return (
    <Show when={props.error}>
      <div class="message error" role="alert">
        <span>{props.error}</span>
        <Action label="Dismiss error" icon="close" onClick={props.onDismiss} />
      </div>
    </Show>
  );
}

export function RestoreBanner(props: {
  label: string | undefined;
  busy: boolean;
  onUndo: () => void;
  onDismiss: () => void;
}) {
  return (
    <Show when={props.label}>
      <div class="message" role="status">
        <span>{props.label}</span>
        <button disabled={props.busy} onClick={props.onUndo}>
          Undo restore
        </button>
        <button onClick={props.onDismiss}>Dismiss</button>
      </div>
    </Show>
  );
}

export function ExternalChangeBanner(props: {
  show: boolean;
  onReload: () => void;
  onSaveAs: () => void;
}) {
  return (
    <Show when={props.show}>
      <div class="message">
        <span>
          This file changed outside River. Reload it, or Save As to keep your
          edits.
        </span>
        <button onClick={props.onReload}>Reload</button>
        <button onClick={props.onSaveAs}>Save As…</button>
      </div>
    </Show>
  );
}
