import { For, Show } from "solid-js";
import { Button } from "@kobalte/core/button";
import type { RecentProject } from "../types";
import { Icon } from "./controls";
function lastOpened(seconds: number) {
  const date = new Date(seconds * 1000),
    today = new Date();
  if (date.toDateString() === today.toDateString()) return "Today";
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);
  if (date.toDateString() === yesterday.toDateString()) return "Yesterday";
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: date.getFullYear() === today.getFullYear() ? undefined : "numeric",
  });
}
export function ProjectWelcome(props: {
  recent: RecentProject[];
  busy: boolean;
  error: string;
  onNew: () => void;
  onOpen: (path?: string) => void;
  /** Remove a location from Recent Projects; the folder is untouched. */
  onForget: (path: string) => void;
  onDismiss: () => void;
}) {
  return (
    <main
      class="project-welcome"
      aria-label="Project welcome"
      aria-busy={props.busy}
    >
      <section class="welcome-window">
        <div class="welcome-intro">
          <svg class="river-mark" viewBox="0 0 80 80" aria-hidden="true">
            <rect width="80" height="80" rx="18" fill="#2d6475" />
            <path
              d="M18 26c14-12 30 12 44 0M18 40c14-12 30 12 44 0M18 54c14-12 30 12 44 0"
              fill="none"
              stroke="#fff"
              stroke-width="4"
              stroke-linecap="round"
            />
          </svg>
          <h1>River Writer</h1>
          <p>A place for your next story.</p>
          <div class="project-start-actions">
            <Button
              aria-label="New Project…"
              disabled={props.busy}
              onClick={props.onNew}
            >
              <Icon name="new-folder" />
              <span>New Project…</span>
              <kbd>⇧⌘N</kbd>
            </Button>
            <Button
              aria-label="Open Project…"
              disabled={props.busy}
              onClick={() => props.onOpen()}
            >
              <Icon name="folder-opened" />
              <span>Open Project…</span>
              <kbd>⇧⌘O</kbd>
            </Button>
          </div>
          <Show when={props.busy}>
            <p class="project-opening" role="status">
              Opening project…
            </p>
          </Show>
        </div>
        <section class="recent-projects" aria-label="Recent projects">
          <h2>Recent Projects</h2>
          <Show
            when={props.recent.length}
            fallback={
              <div class="no-recent">
                <Icon name="folder" />
                <p>Your projects will appear here.</p>
              </div>
            }
          >
            <ul>
              <For each={props.recent}>
                {(project) => (
                  <li>
                    <Button
                      class="recent-project"
                      disabled={props.busy}
                      onClick={() => props.onOpen(project.path)}
                      title={project.path}
                    >
                      <Icon name="folder" />
                      <span class="recent-details">
                        <strong>{project.name}</strong>
                        <span class="recent-path">{project.path}</span>
                      </span>
                      <small>
                        {project.available
                          ? lastOpened(project.lastOpened)
                          : "Unavailable"}
                      </small>
                    </Button>
                    <Button
                      class="forget-recent"
                      aria-label={`Remove ${project.name} from Recent Projects`}
                      title="Remove from Recent Projects"
                      disabled={props.busy}
                      onClick={() => props.onForget(project.path)}
                    >
                      <Icon name="close" />
                    </Button>
                  </li>
                )}
              </For>
            </ul>
          </Show>
        </section>
      </section>
      <Show when={props.error}>
        <div class="welcome-error" role="alert">
          <span>{props.error}</span>
          <Button aria-label="Dismiss error" onClick={props.onDismiss}>
            <Icon name="close" />
          </Button>
        </div>
      </Show>
    </main>
  );
}
