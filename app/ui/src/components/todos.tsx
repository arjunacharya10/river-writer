import { For, Show, createEffect, createSignal, on, onCleanup } from "solid-js";
import { Checkbox } from "@kobalte/core/checkbox";
import { call } from "../bridge";
import { Action, Icon } from "./controls";

type Task = { id: number; text: string; completed: boolean };
type Snapshot = { revision: string | null; tasks: Task[] };
type Change =
  | { kind: "add"; text: string }
  | { kind: "rename"; id: number; text: string }
  | { kind: "toggle" | "delete"; id: number }
  | { kind: "move"; id: number; before: number | null };
export function TodosPanel(props: {
  root: string;
  active: boolean;
  onError: (e: unknown) => void;
  registerFlush: (flush: () => Promise<void>) => () => void;
}) {
  const root = props.root;
  const [snapshot, setSnapshot] = createSignal<Snapshot | null>(null);
  const [draft, setDraft] = createSignal("");
  const [editing, setEditing] = createSignal<{
    id: number;
    text: string;
  } | null>(null);
  const [busy, setBusy] = createSignal(false),
    [error, setError] = createSignal("");
  const [dragged, setDragged] = createSignal<number | null>(null);
  let latest: Snapshot | null = null,
    disposed = false,
    loading = false;
  let queue: Promise<void> = Promise.resolve(),
    adding: Promise<void> | null = null;
  let entry!: HTMLInputElement;
  const pending = () =>
    snapshot()?.tasks.filter((task) => !task.completed) ?? [];
  const completed = () =>
    snapshot()?.tasks.filter((task) => task.completed) ?? [];
  const fail = (e: unknown) => {
    if (!disposed) setError(String(e));
    else props.onError(e);
  };
  async function load() {
    if (loading) return;
    loading = true;
    setBusy(true);
    try {
      await queue.catch(() => {});
      latest = await call<Snapshot>("load_todos", { root });
      queue = Promise.resolve();
      if (!disposed) {
        setSnapshot(latest);
        setError("");
      }
    } catch (e) {
      fail(e);
    } finally {
      loading = false;
      if (!disposed) setBusy(false);
    }
  }
  function mutate(change: Change) {
    setBusy(true);
    const task = queue
      .catch(() => {})
      .then(async () => {
        if (!latest) throw Error("Reload your tasks first.");
        latest = await call<Snapshot>("update_todos", {
          root,
          expected: latest.revision,
          change,
        });
        if (!disposed) {
          setSnapshot(latest);
          setError("");
        }
      });
    queue = task;
    void task.then(
      () => {
        if (!disposed) setBusy(false);
      },
      () => {
        if (!disposed) setBusy(false);
      },
    );
    return task;
  }
  function add() {
    if (adding) return adding;
    const text = draft().trim();
    if (!text) return Promise.resolve();
    adding = mutate({ kind: "add", text })
      .then(() => {
        if (draft().trim() === text) setDraft("");
      })
      .finally(() => {
        adding = null;
      });
    return adding;
  }
  async function commitEdit() {
    const edit = editing();
    if (!edit) return;
    setEditing(null);
    if (
      snapshot()?.tasks.find((task) => task.id === edit.id)?.text ===
      edit.text.trim()
    )
      return;
    try {
      await mutate({ kind: "rename", ...edit });
    } catch (e) {
      if (!disposed && !editing()) setEditing(edit);
      throw e;
    }
  }
  async function flush() {
    // Wait for an in-flight action, then retry any text still in an input.
    // An old failed checkbox/delete must not trap the user in this project.
    await queue.catch(() => {});
    await commitEdit();
    await add();
  }
  const unregister = props.registerFlush(flush);
  createEffect(
    on(
      () => props.active,
      (active) => {
        if (active && !editing() && !draft() && !busy()) void load();
        if (!active) void flush().catch(fail);
      },
    ),
  );
  const focus = () => {
    if (props.active && !busy() && !editing() && !draft()) void load();
  };
  window.addEventListener("focus", focus);
  onCleanup(() => {
    unregister();
    window.removeEventListener("focus", focus);
    void flush().catch(props.onError);
    disposed = true;
  });
  async function reload() {
    const edit = editing();
    if (edit && !draft()) setDraft(edit.text);
    setEditing(null);
    await load();
  }
  function move(id: number, direction: number) {
    const tasks = pending(),
      index = tasks.findIndex((task) => task.id === id);
    if (
      index < 0 ||
      (direction < 0 && index === 0) ||
      (direction > 0 && index === tasks.length - 1)
    )
      return;
    const before =
      direction < 0 ? tasks[index - 1].id : (tasks[index + 2]?.id ?? null);
    void mutate({ kind: "move", id, before })
      .then(() => {
        queueMicrotask(() =>
          document
            .querySelector<HTMLButtonElement>(
              `[data-task-id="${direction < 0 ? tasks[index - 1].id : tasks[index + 1].id}"] .task-grip`,
            )
            ?.focus(),
        );
      })
      .catch(fail);
  }
  function row(task: Task) {
    return (
      <div
        class={`task-row ${task.completed ? "completed" : ""}`}
        data-task-id={task.id}
        onDragOver={(event) => {
          if (dragged() !== null && !task.completed) event.preventDefault();
        }}
        onDrop={(event) => {
          event.preventDefault();
          const id = dragged();
          setDragged(null);
          if (id !== null && id !== task.id && !task.completed)
            void mutate({ kind: "move", id, before: task.id }).catch(fail);
        }}
      >
        <Show when={!task.completed}>
          <button
            class="task-grip"
            aria-label={`Reorder ${task.text}`}
            title="Drag to reorder. Option + ↑ or ↓ to move."
            disabled={busy()}
            draggable={!busy()}
            onDragStart={(event) => {
              setDragged(task.id);
              event.dataTransfer?.setData("text/plain", String(task.id));
            }}
            onDragEnd={() => setDragged(null)}
            onKeyDown={(event) => {
              if (
                event.altKey &&
                ["ArrowUp", "ArrowDown"].includes(event.key)
              ) {
                event.preventDefault();
                move(task.id, event.key === "ArrowUp" ? -1 : 1);
              }
            }}
          >
            <Icon name="gripper" />
          </button>
        </Show>
        <Checkbox
          class="task-check"
          checked={task.completed}
          disabled={busy()}
          onChange={() =>
            void mutate({ kind: "toggle", id: task.id })
              .then(() => queueMicrotask(() => entry?.focus()))
              .catch(fail)
          }
        >
          <Checkbox.Input
            aria-label={`${task.completed ? "Reopen" : "Complete"} ${task.text || "empty task"}`}
          />
          <Checkbox.Control class="task-check-control">
            <Checkbox.Indicator>
              <Icon name="check" />
            </Checkbox.Indicator>
          </Checkbox.Control>
        </Checkbox>
        <Show
          when={editing()?.id === task.id}
          fallback={
            <button
              class="task-title"
              disabled={busy()}
              onClick={() =>
                void add()
                  .then(() => setEditing({ id: task.id, text: task.text }))
                  .catch(fail)
              }
            >
              {task.text || "Empty task"}
            </button>
          }
        >
          <input
            class="task-edit"
            aria-label="Edit task"
            value={editing()?.text ?? ""}
            ref={(node) =>
              queueMicrotask(() => {
                node.focus();
                node.select();
              })
            }
            onInput={(event) =>
              setEditing({ id: task.id, text: event.currentTarget.value })
            }
            onBlur={() => void commitEdit().catch(fail)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void commitEdit().catch(fail);
              }
              if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                setEditing(null);
              }
            }}
          />
        </Show>
        <Action
          label={`Delete ${task.text || "empty task"}`}
          icon="trash"
          disabled={busy()}
          onClick={() =>
            void mutate({ kind: "delete", id: task.id }).catch(fail)
          }
        />
      </div>
    );
  }
  return (
    <section
      class="todos-panel"
      aria-label="Todos"
      style={{ display: props.active ? undefined : "none" }}
      onKeyDown={(event) => {
        if (
          (event.metaKey || event.ctrlKey) &&
          event.key.toLowerCase() === "s"
        ) {
          event.preventDefault();
          event.stopPropagation();
          void flush().catch(fail);
        }
      }}
    >
      <form
        class="task-entry"
        onSubmit={(event) => {
          event.preventDefault();
          void add()
            .then(() => entry.focus())
            .catch(fail);
        }}
      >
        <Icon name="add" />
        <input
          ref={entry}
          aria-label="Add a task"
          onBlur={() => {
            if (!busy()) void add().catch(fail);
          }}
          placeholder="Add a task"
          value={draft()}
          disabled={!snapshot() || busy()}
          onInput={(event) => setDraft(event.currentTarget.value)}
        />
        <Show when={draft().trim()}>
          <button type="submit" disabled={busy()} aria-label="Add task">
            <Icon name="check" />
          </button>
        </Show>
      </form>
      <Show when={error()}>
        <div class="task-error" role="alert">
          <span>{error()}</span>
          <button onClick={() => void reload()}>Reload tasks</button>
        </div>
      </Show>
      <Show
        when={snapshot()}
        fallback={
          <p class="muted">
            {error() ? "Tasks are unavailable." : "Loading tasks…"}
          </p>
        }
      >
        <div class="task-list" aria-label="Open tasks">
          <For each={pending()}>{row}</For>
        </div>
        <Show when={!pending().length}>
          <p class="muted">
            {completed().length ? "All done." : "Keep your next steps here."}
          </p>
        </Show>
        <Show when={dragged() !== null}>
          <div
            class="task-drop-end"
            onDragOver={(event) => event.preventDefault()}
            onDrop={(event) => {
              event.preventDefault();
              const id = dragged();
              setDragged(null);
              if (id !== null)
                void mutate({ kind: "move", id, before: null }).catch(fail);
            }}
          >
            Move to end
          </div>
        </Show>
        <Show when={completed().length}>
          <details class="completed-tasks">
            <summary>Completed ({completed().length})</summary>
            <For each={completed()}>{row}</For>
          </details>
        </Show>
      </Show>
    </section>
  );
}
