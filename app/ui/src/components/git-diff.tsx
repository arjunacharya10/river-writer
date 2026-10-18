import { For, Show } from "solid-js";
import { Action } from "./controls";
import type { GitWorkflow } from "../features/git-workflow";

/** A commit's (or the working tree's) diff in the side pane, with restore. */
export function GitDiffPane(props: {
  git: GitWorkflow;
  run: (job: () => unknown) => () => void;
}) {
  const git = props.git;
  return (
    <Show when={git.diff()}>
      {(diff) => (
        <section class="git-diff" aria-label="Git diff">
          <header>
            <strong>{diff().title}</strong>
            <Action
              label="Close Git diff"
              icon="close"
              onClick={() => git.setDiff(null)}
            />
          </header>
          <Show when={git.ask() !== undefined}>
            <div
              class="git-confirm"
              role="alertdialog"
              aria-label="Confirm restore"
            >
              <p>
                {`Replace ${git.ask()} with its version in this commit? You can undo this.`}
              </p>
              <button
                class="primary"
                disabled={git.busy()}
                onClick={props.run(() => {
                  const path = git.ask();
                  if (path !== undefined) return git.restore(path);
                })}
              >
                Restore
              </button>
              <button
                disabled={git.busy()}
                onClick={() => git.setAsk(undefined)}
              >
                Cancel
              </button>
            </div>
          </Show>
          <Show when={diff().commit}>
            <ul class="git-diff-files" aria-label="Files in this commit">
              <For each={diff().review.files}>
                {(file) => (
                  <li>
                    <span title={file.path}>{file.path}</span>
                    <small>{file.kind}</small>
                    <button
                      disabled={git.busy()}
                      aria-label={`Restore ${file.path}`}
                      onClick={() => git.setAsk(file.path)}
                    >
                      Restore
                    </button>
                  </li>
                )}
              </For>
            </ul>
          </Show>
          <Show when={diff().review.truncated}>
            <p class="git-message">
              This diff is too large to show in full. Use another Git tool to
              review the remainder.
            </p>
          </Show>
          <pre aria-label="Git changes">
            <For each={diff().review.patch.split("\n")}>
              {(line) => (
                <span
                  class={
                    line.startsWith("+")
                      ? "added"
                      : line.startsWith("-")
                        ? "removed"
                        : ""
                  }
                >
                  {line}
                  {"\n"}
                </span>
              )}
            </For>
          </pre>
        </section>
      )}
    </Show>
  );
}
