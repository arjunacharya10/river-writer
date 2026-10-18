import { For, Show } from "solid-js";
import { Action } from "./controls";
import type { Timeline } from "../features/timeline";

/** Local history of the active story, in the side pane. */
export function TimelinePane(props: {
  timeline: Timeline;
  saved: boolean;
  run: (job: () => unknown) => () => void;
}) {
  const t = props.timeline;
  return (
    <Show when={t.open()}>
      <section class="timeline" aria-label="Local history">
        <header>
          <strong>TIMELINE · LOCAL HISTORY</strong>
          <Action
            label="Close local history"
            icon="close"
            onClick={() => t.setOpen(false)}
          />
        </header>
        <div class="timeline-body">
          <div class="timeline-list">
            <For each={t.versions()}>
              {(version) => (
                <button
                  class={t.version() === version.id ? "selected" : ""}
                  aria-pressed={t.version() === version.id}
                  onClick={props.run(() => t.inspect(version.id))}
                >
                  {new Date(version.timestamp * 1000).toLocaleString()} ·{" "}
                  {version.bytes} bytes
                </button>
              )}
            </For>
            <Show when={t.more()}>
              <button
                disabled={t.busy()}
                onClick={props.run(() => t.load(true))}
              >
                Load older versions
              </button>
            </Show>
            <Show when={!t.versions().length}>
              <p class="muted">
                {t.busy()
                  ? "Loading…"
                  : props.saved
                    ? "Saved versions will appear here."
                    : "Save this draft to start local history."}
              </p>
            </Show>
          </div>
          <div class="timeline-version">
            <Show
              when={t.version()}
              fallback={<p class="muted">Choose a version to read its text.</p>}
            >
              <button onClick={props.run(t.restore)}>
                Restore this version
              </button>
              <pre aria-label="Version text">{t.text()}</pre>
            </Show>
          </div>
        </div>
      </section>
    </Show>
  );
}
