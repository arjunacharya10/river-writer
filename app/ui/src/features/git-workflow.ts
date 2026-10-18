import { createEffect, createSignal, on } from "solid-js";
import { call } from "../bridge";
import type { GitReview } from "../components/source-control";

export type Restored = { before: string; after: string; paths: string[] };
export type GitDiff = { title: string; review: GitReview; commit?: string };

/** Git actions that rewrite working files (restore, undo, branch switch) and
 * the diff shown in the side pane. The workspace supplies the save-before and
 * reload-after steps, so open tabs never disagree with disk. */
export function createGitWorkflow(deps: {
  folder: () => string;
  /** Save what can be saved; throw if a project tab still has unsaved text. */
  prepare: (root: string) => Promise<void>;
  /** Reload tabs/extras for rewritten paths. */
  afterWrite: (root: string, paths: string[]) => Promise<void>;
  /** Close the other side-pane views before showing a diff. */
  makeRoom: () => void;
  notify: (text: string) => void;
}) {
  const [diff, setDiff] = createSignal<GitDiff | null>(null);
  /** `undefined`: no confirmation; string: one file. */
  const [ask, setAsk] = createSignal<string | undefined>(undefined);
  const [busy, setBusy] = createSignal(false);
  const [undo, setUndo] = createSignal<{
    root: string;
    restored: Restored;
    label: string;
  } | null>(null);
  /** Bumped after River rewrites files, so Source Control refreshes at once. */
  const [epoch, setEpoch] = createSignal(0);
  const [branch, setBranch] = createSignal("");

  createEffect(
    on(deps.folder, (root) => {
      setBranch("");
      if (!root) return;
      void call<{ name: string; current: boolean }[]>("git_branches", { root })
        .then((list) => {
          if (deps.folder() === root)
            setBranch(list.find((b) => b.current)?.name ?? "");
        })
        .catch(() => {}); // Not a Git project yet.
    }),
  );

  async function exclusive(job: () => Promise<void>) {
    if (busy()) return;
    setBusy(true);
    try {
      await job();
    } finally {
      setBusy(false);
    }
  }
  function show(title: string, review: GitReview, commit?: string) {
    deps.makeRoom();
    setAsk(undefined);
    setDiff({ title, review, commit });
  }
  const switchBranch = (name: string) =>
    exclusive(async () => {
      const root = deps.folder();
      if (!root) return;
      await deps.prepare(root);
      const paths = await call<string[]>("git_switch_branch", { root, name });
      // Restore undo and commit diffs belong to the branch being left.
      setUndo(null);
      setDiff(null);
      setBranch(name);
      await deps.afterWrite(root, paths);
      deps.notify(`Switched to ${name}.`);
    });
  const restore = (path: string) =>
    exclusive(async () => {
      const shown = diff(),
        root = deps.folder();
      if (!shown?.commit || !root) return;
      await deps.prepare(root);
      const restored = await call<Restored>("git_restore", {
        root,
        commit: shown.commit,
        path,
      });
      setAsk(undefined);
      await deps.afterWrite(root, restored.paths);
      setUndo({
        root,
        restored,
        label: `Restored ${path} from “${shown.title}”.`,
      });
    });
  const undoRestore = () =>
    exclusive(async () => {
      const last = undo();
      if (!last) return;
      await deps.prepare(last.root);
      await call("git_undo_restore", {
        root: last.root,
        restored: last.restored,
      });
      setUndo(null);
      await deps.afterWrite(last.root, last.restored.paths);
      deps.notify("Restore undone.");
    });
  return {
    diff,
    setDiff,
    ask,
    setAsk,
    busy,
    undo,
    setUndo,
    epoch,
    bump: () => setEpoch((n) => n + 1),
    branch,
    setBranch,
    show,
    switchBranch,
    restore,
    undoRestore,
  };
}
export type GitWorkflow = ReturnType<typeof createGitWorkflow>;
