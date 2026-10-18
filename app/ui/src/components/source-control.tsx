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
  identity: { name: string; email: string } | null;
  blocked: string | null;
  files: GitReview["files"];
};
/** A snapshot built on demand; commit only proceeds if the project still matches it. */
type Reviewed = { head: string | null; tree: string; review: GitReview };
export function SourceControl(props: {
  root: string;
  active: boolean;
  beforeCommit: () => Promise<void>;
  onReview: (title: string, review: GitReview, commit?: string) => void;
  onBranch: (name: string) => void;
  /** Bumped after River rewrites files (restore, switch), so counts update at once. */
  refreshKey: number;
}) {
  const root = props.root;
  const [status, setStatus] = createSignal<Status>();
  let reviewedChanges: Reviewed | undefined;
  const [commits, setCommits] = createSignal<Commit[]>([]);
  const [message, setMessage] = createSignal("");
  const [name, setName] = createSignal("");
  const [email, setEmail] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  const [notice, setNotice] = createSignal("");
  const [more, setMore] = createSignal(false);
  let disposed = false;
  // Background checks never disable the panel; only the writer's own actions do.
  let reading = false,
    again = false;
  async function refresh() {
    if (busy() || disposed) return;
    if (reading) {
      again = true;
      return;
    }
    reading = true;
    try {
      let next = await call<Status>("git_status", { root });
      // Projects always have Git (#62). If setup did not happen on open, do it
      // now rather than asking the writer to.
      if (!next.initialized) {
        await call("git_init", { root });
        next = await call<Status>("git_status", { root });
      }
      const log = next.initialized
        ? await call<Commit[]>("git_log", { root })
        : [];
      if (disposed) return;
      props.onBranch(next.initialized ? next.branch : "");
      if (!status() || status()?.head !== next.head) {
        setCommits(log);
        setMore(log.length === 100);
      }
      setStatus(next);
    } catch (e) {
      if (!disposed) setError(String(e));
    } finally {
      reading = false;
      if (again) {
        again = false;
        void refresh();
      }
    }
  }
  async function act(job: () => Promise<void>) {
    if (busy()) return;
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
    await props.beforeCommit();
    const reviewed =
      reviewedChanges ?? (await call<Reviewed>("git_review", { root }));
    await call("git_commit", {
      root,
      head: reviewed.head,
      tree: reviewed.tree,
      message: message(),
    });
    reviewedChanges = undefined;
    setMessage("");
    setNotice("Committed.");
  }
  createEffect(
    on(
      () => props.active,
      (active) => {
        if (active) void refresh();
      },
    ),
  );
  createEffect(
    on(
      () => props.refreshKey,
      () => {
        reviewedChanges = undefined;
        void refresh();
      },
      { defer: true },
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
          disabled={busy()}
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
              fallback={<p class="muted">Setting up version history…</p>}
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
                    disabled={busy()}
                  />
                  <input
                    aria-label="Git email"
                    placeholder="Email"
                    type="email"
                    required
                    value={email()}
                    onInput={(e) => setEmail(e.currentTarget.value)}
                    disabled={busy()}
                  />
                  <button disabled={busy()}>Save Git identity</button>
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
                  disabled={busy()}
                  rows={3}
                />
                <button
                  class="primary"
                  disabled={
                    busy() ||
                    !!s().blocked ||
                    !s().identity ||
                    !message().trim() ||
                    !s().files.length
                  }
                >
                  Commit all changes
                </button>
              </form>
              <h3>CHANGES · {s().files.length}</h3>
              <Show
                when={s().files.length}
                fallback={<p class="muted">No changes to commit.</p>}
              >
                <button
                  class="git-item"
                  disabled={busy()}
                  onClick={() =>
                    void act(async () => {
                      const reviewed = await call<Reviewed>("git_review", {
                        root,
                      });
                      reviewedChanges = reviewed;
                      props.onReview("Uncommitted changes", reviewed.review);
                    })
                  }
                >
                  Review changes
                </button>
                <For each={s().files}>
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
                    disabled={busy()}
                    title={`${c.id}\n${c.author}`}
                    onClick={() =>
                      void act(async () => {
                        const diff = await call<GitReview>("git_diff", {
                          root,
                          id: c.id,
                        });
                        if (!disposed)
                          props.onReview(c.message.split("\n")[0], diff, c.id);
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
                  disabled={busy()}
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
