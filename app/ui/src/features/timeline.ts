import { createEffect, createSignal, on } from "solid-js";
import { isolateHistory } from "@codemirror/commands";
import type { EditorView } from "@codemirror/view";
import { call } from "../bridge";
import type { Story } from "../types";

export type Version = { id: string; timestamp: number; bytes: number };

/** Local history (Timeline) for the active story: list, read and restore versions. */
export function createTimeline(deps: {
  current: () => Story | undefined;
  active: () => number | null;
  sync: (id: number) => Promise<void>;
  view: () => EditorView | undefined;
  notify: (text: string) => void;
  fail: (e: unknown) => void;
}) {
  const [open, setOpen] = createSignal(false);
  const [versions, setVersions] = createSignal<Version[]>([]);
  const [version, setVersion] = createSignal("");
  const [text, setText] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [more, setMore] = createSignal(false);
  // Tickets drop replies that arrive after the tab or request changed.
  let listRequest = 0;
  let readRequest = 0;

  async function load(older = false) {
    const doc = deps.current();
    const ticket = ++listRequest;
    if (!doc?.path) {
      setVersions([]);
      setBusy(false);
      setMore(false);
      return;
    }
    setBusy(true);
    try {
      const items = await call<Version[]>("local_history", {
        id: doc.id,
        before: older ? versions().at(-1)?.id : null,
      });
      if (ticket === listRequest && deps.active() === doc.id) {
        setVersions((previous) => (older ? [...previous, ...items] : items));
        setMore(items.length === 200);
      }
    } finally {
      if (ticket === listRequest) setBusy(false);
    }
  }
  async function inspect(id: string) {
    const doc = deps.current();
    if (!doc) return;
    const ticket = ++readRequest;
    setVersion("");
    const body = await call<string>("history_text", {
      id: doc.id,
      version: id,
    });
    if (ticket !== readRequest || deps.active() !== doc.id) return;
    setText(body);
    setVersion(id);
  }
  /** Replace the editor text with the chosen version as one undoable edit. */
  async function restore() {
    const doc = deps.current(),
      chosen = version();
    if (!doc || !chosen) return;
    await deps.sync(doc.id);
    const body = await call<string>("prepare_restore", {
      id: doc.id,
      revision: doc.revision,
      version: chosen,
    });
    if (deps.active() !== doc.id || deps.current()?.revision !== doc.revision)
      throw Error("The story changed. Choose the version again.");
    const view = deps.view();
    if (view && view.state.doc.toString() !== body) {
      view.dispatch({
        annotations: isolateHistory.of("full"),
        changes: { from: 0, to: view.state.doc.length, insert: body },
      });
      view.focus();
    }
    deps.notify("Version restored. Undo returns to your previous text.");
    await load();
  }
  function reset() {
    listRequest++;
    readRequest++;
    setVersion("");
    setText("");
    setVersions([]);
  }
  createEffect(
    on([deps.active, open], ([, isOpen]) => {
      readRequest++;
      setVersion("");
      setText("");
      setVersions([]);
      setMore(false);
      if (isOpen) void load().catch(deps.fail);
    }),
  );
  return {
    open,
    setOpen,
    versions,
    version,
    text,
    busy,
    more,
    load,
    inspect,
    restore,
    reset,
  };
}
export type Timeline = ReturnType<typeof createTimeline>;
