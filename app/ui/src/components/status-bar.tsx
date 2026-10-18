import { For, Show } from "solid-js";
import { Icon } from "./controls";
import type { Analysis } from "../types";

export function StatusBar(props: {
  branch: string;
  onBranch: () => void;
  notice: string;
  /** Present when a story is open. */
  story: { analysis: Analysis | null } | null;
}) {
  return (
    <footer class="status-bar">
      <Show when={props.branch}>
        <button
          class="status-branch"
          title="Source Control"
          onClick={props.onBranch}
        >
          <Icon name="git-branch" />
          {props.branch}
        </button>
      </Show>
      <span>{props.notice || "River Writer"}</span>
      <div class="status-spacer" />
      <Show when={props.story}>
        {(story) => (
          <>
            <span>{story().analysis?.words ?? "—"} words</span>
            <span>{story().analysis?.scenes ?? "—"} scenes</span>
          </>
        )}
      </Show>
    </footer>
  );
}

/** Parser problems for the active story; click to jump to the source. */
export function Problems(props: {
  items: Analysis["diagnostics"] | undefined;
  onJump: (at: number) => void;
}) {
  return (
    <Show when={props.items?.length}>
      <div class="problems" aria-label="Problems">
        <For each={props.items}>
          {(d) => (
            <button onClick={() => props.onJump(d.at)}>
              <Icon name={d.severity === "Error" ? "error" : "warning"} />
              {d.message}
            </button>
          )}
        </For>
      </div>
    </Show>
  );
}
