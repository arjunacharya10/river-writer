import { For, Show, createEffect, createSignal, onCleanup, on } from "solid-js";
import { call } from "../bridge";
import { Action } from "./controls";
export type GitReview = {
  files: { path: string; kind: string }[];
  patch: string;
  truncated: boolean;
};
type Commit = {
  id: string;
  message: string;
  author: string;
  timestamp: number;
};
type Status = {
  initialized: boolean;
  branch: string;
  head: string | null;
  tree: string;
  identity: { name: string; email: string } | null;
  blocked: string | null;
  review: GitReview;
};
export function SourceControl(props: {
  root: string;
  active: boolean;
  beforeCommit: () => Promise<void>;
  onReview: (title: string, review: GitReview) => void;
}) {
  const root = props.root;
  const [status, setStatus] = createSignal<Status>();
  let reviewedChanges: Status | undefined;
  const [commits, setCommits] = createSignal<Commit[]>([]);
  const [message, setMessage] = createSignal("");
  const [name, setName] = createSignal("");
  const [email, setEmail] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  const [notice, setNotice] = createSignal("");
  const [more, setMore] = createSignal(false);
  let disposed = false;
  const [reading, setReading] = createSignal(false);
  async function refresh() {
    if (reading() || busy() || disposed) return;
    setReading(true);
    try {
      const next = await call<Status>("git_status", { root });
      const log = next.initialized
        ? await call<Commit[]>("git_log", { root })
        : [];
      if (disposed) return;
      if (!status() || status()?.head !== next.head) {
        setCommits(log);
        setMore(log.length === 100);
      }
      setStatus(next);
    } catch (e) {
      if (!disposed) setError(String(e));
    } finally {
      setReading(false);
    }
  }
  async function act(job: () => Promise<void>) {
    if (busy() || reading()) return;
    setBusy(true);
    setError("");
    setNotice("");
    try {
      await job();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      await refresh();
    }
  }
  async function commit() {
    const reviewed = reviewedChanges ?? status();
    if (!reviewed) return;
    await props.beforeCommit();
    await call("git_commit", {
      root,
      head: reviewed.head,
      tree: reviewed.tree,
      message: message(),
    });
    reviewedChanges = undefined;
    setMessage("");
    setNotice("Committed locally. Push to your remote when ready.");
  }
  createEffect(
    on(
      () => props.active,
      (active) => {
        if (active) void refresh();
      },
    ),
  );
  const timer = setInterval(() => {
    if (props.active && !error()) void refresh();
  }, 3000);
  onCleanup(() => {
    disposed = true;
    clearInterval(timer);
  });
  return (
    <section
      class="source-control"
      aria-label="Source Control"
      style={{ display: props.active ? undefined : "none" }}
    >
      <header>
        <strong>{status()?.branch || "Git"}</strong>
        <Action
          label="Refresh Git"
          icon="refresh"
          disabled={busy() || reading()}
          onClick={() => {
            reviewedChanges = undefined;
            setError("");
            void refresh();
          }}
        />
      </header>
      <Show when={error()}>
        <div class="git-message" role="alert">
          {error()}
        </div>
      </Show>
      <Show when={notice()}>
        <p class="git-message" role="status">
          {notice()}
        </p>
      </Show>
      <Show when={status()} fallback={<p class="muted">Loading Git…</p>}>
        {(s) => (
          <>
            <Show
              when={s().initialized}
              fallback={
                <>
                  <p class="muted">
                    Use Git to keep named versions of this project.
                  </p>
                  <button
                    disabled={busy() || reading()}
                    onClick={() =>
                      void act(async () => {
                        await call("git_init", { root });
                      })
                    }
                  >
                    Initialize Git
                  </button>
                </>
              }
            >
              <Show when={s().blocked}>
                <p class="git-message" role="alert">
                  {s().blocked}
                </p>
              </Show>
              <Show when={!s().identity}>
                <form
                  onSubmit={(e) => {
                    e.preventDefault();
                    void act(async () => {
                      await call("git_identity", {
                        root,
                        name: name(),
                        email: email(),
                      });
                    });
                  }}
                >
                  <p>Set your Git identity for this project.</p>
                  <input
                    aria-label="Git name"
                    placeholder="Name"
                    required
                    value={name()}
                    onInput={(e) => setName(e.currentTarget.value)}
                    disabled={busy() || reading()}
                  />
                  <input
                    aria-label="Git email"
                    placeholder="Email"
                    type="email"
                    required
                    value={email()}
                    onInput={(e) => setEmail(e.currentTarget.value)}
                    disabled={busy() || reading()}
                  />
                  <button disabled={busy() || reading()}>
                    Save Git identity
                  </button>
                </form>
              </Show>
              <form
                onSubmit={(e) => {
                  e.preventDefault();
                  void act(commit);
                }}
              >
                <textarea
                  aria-label="Commit message"
                  placeholder="Describe what you changed…"
                  value={message()}
                  onInput={(e) => setMessage(e.currentTarget.value)}
                  disabled={busy() || reading()}
                  rows={3}
                />
                <button
                  class="primary"
                  disabled={
                    busy() ||
                    reading() ||
                    !!s().blocked ||
                    !s().identity ||
                    !message().trim() ||
                    !s().review.files.length
                  }
                >
                  Commit all changes
                </button>
              </form>
              <h3>CHANGES · {s().review.files.length}</h3>
              <Show
                when={s().review.files.length}
                fallback={<p class="muted">No changes to commit.</p>}
              >
                <button
                  class="git-item"
                  disabled={busy() || reading()}
                  onClick={() => {
                    reviewedChanges = s();
                    props.onReview("Uncommitted changes", s().review);
                  }}
                >
                  Review changes
                </button>
                <For each={s().review.files}>
                  {(file) => (
                    <div class="git-file" title={file.path}>
                      <span>{file.path}</span>
                      <small>{file.kind}</small>
                    </div>
                  )}
                </For>
              </Show>
              <h3>COMMITS</h3>
              <For each={commits()}>
                {(c) => (
                  <button
                    class="git-item"
                    disabled={busy() || reading()}
                    title={`${c.id}\n${c.author}`}
                    onClick={() =>
                      void act(async () => {
                        const diff = await call<GitReview>("git_diff", {
                          root,
                          id: c.id,
                        });
                        if (!disposed)
                          props.onReview(c.message.split("\n")[0], diff);
                      })
                    }
                  >
                    <strong>{c.message.split("\n")[0]}</strong>
                    <small>
                      {c.author} ·{" "}
                      {new Date(c.timestamp * 1000).toLocaleString()}
                    </small>
                  </button>
                )}
              </For>
              <Show when={!commits().length}>
                <p class="muted">Your first commit will appear here.</p>
              </Show>
              <Show when={more()}>
                <button
                  disabled={busy() || reading()}
                  onClick={() =>
                    void act(async () => {
                      const older = await call<Commit[]>("git_log", {
                        root,
                        before: commits().at(-1)?.id,
                      });
                      setCommits([...commits(), ...older]);
                      setMore(older.length === 100);
                    })
                  }
                >
                  Load older commits
                </button>
              </Show>
            </Show>
          </>
        )}
      </Show>
    </section>
  );
}
