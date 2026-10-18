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
  switchBranch: (name: string) => Promise<void>;
  onBranch: (name: string) => void;
  /** Bumped after River rewrites files (restore, switch), so counts update at once. */
  refreshKey: number;
}) {
  const [branches, setBranches] = createSignal<
    { name: string; current: boolean }[]
  >([]);
  const [branchName, setBranchName] = createSignal("");
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
  const [reading, setReading] = createSignal(false);
  async function refresh() {
    if (reading() || busy() || disposed) return;
    setReading(true);
    try {
      const next = await call<Status>("git_status", { root });
      const log = next.initialized
        ? await call<Commit[]>("git_log", { root })
        : [];
      const list = next.initialized
        ? await call<{ name: string; current: boolean }[]>("git_branches", {
            root,
          })
        : [];
      if (disposed) return;
      setBranches(list);
      props.onBranch(next.initialized ? next.branch : "");
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
                  disabled={busy() || reading()}
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
              <h3>BRANCHES</h3>
              <For each={branches()}>
                {(b) => (
                  <button
                    class={`git-item git-branch ${b.current ? "current" : ""}`}
                    aria-current={b.current ? "true" : undefined}
                    disabled={busy() || reading() || b.current}
                    title={b.current ? "Current branch" : `Switch to ${b.name}`}
                    onClick={() =>
                      void act(async () => {
                        await props.switchBranch(b.name);
                        setNotice(`Switched to ${b.name}.`);
                      })
                    }
                  >
                    {b.current ? "✓ " : ""}
                    {b.name}
                  </button>
                )}
              </For>
              <form
                class="git-new-branch"
                onSubmit={(e) => {
                  e.preventDefault();
                  const name = branchName().trim();
                  void act(async () => {
                    await call("git_create_branch", { root, name });
                    await props.switchBranch(name);
                    setBranchName("");
                    setNotice(`Created and switched to ${name}.`);
                  });
                }}
              >
                <input
                  aria-label="New branch name"
                  placeholder="New branch, e.g. draft-2"
                  value={branchName()}
                  onInput={(e) => setBranchName(e.currentTarget.value)}
                  disabled={busy() || reading() || !s().head}
                />
                <button
                  disabled={
                    busy() || reading() || !s().head || !branchName().trim()
                  }
                >
                  Create and switch
                </button>
              </form>
              <Show when={!s().head}>
                <p class="muted">Commit once to start new branches.</p>
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
