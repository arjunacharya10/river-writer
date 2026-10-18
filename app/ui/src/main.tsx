import { render } from "solid-js/web";
import {
  For,
  Show,
  createSignal,
  createEffect,
  onMount,
  onCleanup,
} from "solid-js";
import { EditorView } from "@codemirror/view";
import type { EditorState } from "@codemirror/state";
import { openSearchPanel } from "@codemirror/search";
import { call, commands } from "./bridge";
import { stateFor, semantic, byteOffsets } from "./editor";
import {
  Action,
  Icon,
  TemplatePicker,
  Palette,
  Split,
  type Command,
} from "./components/controls";
import { PagePreview } from "./components/preview";
import {
  nameOf,
  type Story,
  type Analysis,
  type Preview,
  type FileEntry,
  type TemplateId,
} from "./types";
import "@vscode/codicons/dist/codicon.css";
import "./style.css";
function App() {
  const [tabs, setTabs] = createSignal<Story[]>([]),
    [active, setActive] = createSignal<number | null>(null),
    [analysis, setAnalysis] = createSignal<Analysis | null>(null);
  const [folder, setFolder] = createSignal(""),
    [files, setFiles] = createSignal<FileEntry[]>([]),
    [sidebar, setSidebar] = createSignal(true),
    [sideView, setSideView] = createSignal("explorer");
  const [previewOpen, setPreviewOpen] = createSignal(false),
    [preview, setPreview] = createSignal<Preview | null>(null),
    [previewBusy, setPreviewBusy] = createSignal(false),
    [previewError, setPreviewError] = createSignal("");
  const [error, setError] = createSignal(""),
    [notice, setNotice] = createSignal(""),
    [external, setExternal] = createSignal(false),
    [palette, setPalette] = createSignal(false),
    [query, setQuery] = createSignal("");
  const system = matchMedia("(prefers-color-scheme: dark)");
  const [dark, setDark] = createSignal(system.matches);
  let manualTheme = false;
  let host!: HTMLDivElement,
    view: EditorView | undefined,
    loaded: number | null = null,
    disposed = false,
    previewRequest = 0,
    refreshTimer: ReturnType<typeof setTimeout> | undefined,
    checking = false,
    quitting = false;
  const states = new Map<number, EditorState>(),
    scrolls = new Map<number, number>(),
    sent = new Map<number, number>(),
    queues = new Map<number, Promise<unknown>>();
  const current = () => tabs().find((d) => d.id === active());
  const fail = (e: unknown) => {
    setError(String(e));
  };
  const patch = (id: number, changes: Partial<Story>) =>
    setTabs((ds) => ds.map((d) => (d.id === id ? { ...d, ...changes } : d)));
  function applyAnalysis(a: Analysis, id: number) {
    if (active() !== id || current()?.revision !== a.revision) return;
    patch(id, { dirty: a.dirty });
    setAnalysis(a);
    view?.dispatch({
      effects: semantic.of({
        analysis: a,
        script: current()?.template !== "manuscript",
      }),
    });
  }
  async function sync(id: number) {
    const doc = tabs().find((d) => d.id === id);
    if (!doc) return;
    const previous = queues.get(id) ?? Promise.resolve();
    const task = previous
      .catch(() => {})
      .then(async () => {
        if ((sent.get(id) ?? 0) >= doc.revision) return;
        const a = await call<Analysis>("sync_document", {
          id,
          revision: doc.revision,
          text: doc.text,
          template: doc.template,
        });
        sent.set(id, doc.revision);
        applyAnalysis(a, id);
      });
    queues.set(id, task);
    await task;
  }
  async function updatePreview() {
    const doc = current();
    if (!doc || !previewOpen()) return;
    const ticket = ++previewRequest;
    setPreviewBusy(true);
    setPreviewError("");
    try {
      await Promise.all(tabs().map((d) => sync(d.id)));
      const result = await call<Preview>("paginate", { id: doc.id });
      if (
        ticket === previewRequest &&
        active() === doc.id &&
        current()?.revision === result.revision
      )
        setPreview(result);
    } catch (e) {
      if (ticket === previewRequest) setPreviewError(String(e));
    } finally {
      if (ticket === previewRequest) setPreviewBusy(false);
    }
  }
  function schedule(id: number) {
    clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => {
      void sync(id)
        .then(() => updatePreview())
        .catch(fail);
    }, 120);
  }
  function changed(id: number, text: string) {
    const doc = tabs().find((d) => d.id === id);
    if (!doc) return;
    patch(id, { text, revision: doc.revision + 1, dirty: true });
    setNotice("");
    schedule(id);
  }
  function select(id: number) {
    if (view && loaded !== null) {
      states.set(loaded, view.state);
      scrolls.set(loaded, view.scrollDOM.scrollTop);
      void sync(loaded).catch(fail);
    }
    setActive(id);
  }
  async function add(doc: Story) {
    if (!tabs().some((d) => d.id === doc.id)) setTabs((ds) => [...ds, doc]);
    sent.set(doc.id, doc.revision);
    select(doc.id);
  }
  async function newFile() {
    await add(await call<Story>("new_document"));
  }
  async function open(path?: string) {
    const doc = await call<Story | null>("open_document", {
      path: path ?? null,
    });
    if (doc) await add(doc);
  }
  async function openFolder() {
    const result = await call<[string, FileEntry[]] | null>("open_folder");
    if (result) {
      setFolder(result[0]);
      setFiles(result[1]);
      setSidebar(true);
      setSideView("explorer");
    }
  }
  async function save(saveAs = false) {
    const doc = current();
    if (!doc) return false;
    await sync(doc.id);
    const result = await call<Story | null>("save_document", {
      id: doc.id,
      saveAs,
    });
    if (!result) return false;
    const local = tabs().find((d) => d.id === doc.id);
    patch(doc.id, {
      path: result.path,
      dirty: local?.revision !== result.revision,
      template: result.template,
    });
    setNotice("Saved");
    setExternal(false);
    await call("persist_saved_template", { id: doc.id });
    // Saving a new .fountain file changes syntax, without changing its text or history.
    if (active() === doc.id)
      applyAnalysis(
        await call<Analysis>("analyze_document", { id: doc.id }),
        doc.id,
      );
    if (folder())
      setFiles(await call<FileEntry[]>("refresh_folder", { path: folder() }));
    return true;
  }
  async function close(id: number) {
    const doc = tabs().find((d) => d.id === id);
    if (!doc) return;
    if (
      doc.dirty &&
      !(await call<boolean>("confirm_discard", { name: nameOf(doc) }))
    )
      return;
    await queues.get(id)?.catch(() => {});
    await call("close_document", { id });
    const remaining = tabs().filter((d) => d.id !== id);
    states.delete(id);
    scrolls.delete(id);
    sent.delete(id);
    queues.delete(id);
    setTabs(remaining);
    if (active() === id) {
      loaded = null;
      setActive(remaining.at(-1)?.id ?? null);
    }
  }
  async function quit() {
    if (quitting) return;
    quitting = true;
    try {
      for (const d of tabs()) {
        if (
          d.dirty &&
          !(await call<boolean>("confirm_discard", { name: nameOf(d) }))
        )
          return;
      }
      await call("quit_confirmed");
    } finally {
      quitting = false;
    }
  }
  async function reload() {
    const doc = current();
    if (!doc) return;
    if (
      doc.dirty &&
      !(await call<boolean>("confirm_discard", { name: nameOf(doc) }))
    )
      return;
    await sync(doc.id);
    const d = await call<Story>("reload_document", { id: doc.id });
    patch(d.id, d);
    sent.set(d.id, d.revision);
    states.delete(d.id);
    loaded = null;
    loadEditor();
    setExternal(false);
  }
  async function changeTemplate(template: TemplateId) {
    const d = current();
    if (!d || template === d.template) return;
    patch(d.id, { template, revision: d.revision + 1 });
    await sync(d.id);
    await call("persist_template", { id: d.id });
    await updatePreview();
  }
  async function exportPdf() {
    const d = current();
    if (!d) return;
    await Promise.all(tabs().map((d) => sync(d.id)));
    const path = await call<string | null>("export_pdf", { id: d.id });
    if (path) setNotice(`Exported ${path.split("/").pop()}`);
  }
  function jump(bytes: number) {
    const doc = current();
    if (!doc || !view) return;
    const pos = byteOffsets(doc.text)[bytes] ?? 0;
    view.dispatch({
      selection: { anchor: pos },
      effects: EditorView.scrollIntoView(pos, { y: "center" }),
    });
    view.focus();
  }
  function togglePreview() {
    setPreviewOpen((v) => !v);
    if (previewOpen()) void updatePreview();
    else {
      previewRequest++;
      setPreviewBusy(false);
    }
  }
  function toggleTheme() {
    manualTheme = true;
    setDark((v) => !v);
  }
  function find() {
    if (view) {
      openSearchPanel(view);
      view.focus();
    }
  }
  const run = (job: () => unknown) => () => {
    try {
      void Promise.resolve(job()).catch(fail);
    } catch (e) {
      fail(e);
    }
  };
  const allCommands: Command[] = [
    { id: "new", label: "File: New File", key: "⌘N", run: run(newFile) },
    { id: "open", label: "File: Open…", key: "⌘O", run: run(() => open()) },
    {
      id: "folder",
      label: "File: Open Folder…",
      key: "⇧⌘O",
      run: run(openFolder),
    },
    { id: "save", label: "File: Save", key: "⌘S", run: run(() => save()) },
    {
      id: "save-as",
      label: "File: Save As…",
      key: "⇧⌘S",
      run: run(() => save(true)),
    },
    {
      id: "close",
      label: "File: Close Tab",
      key: "⌘W",
      run: run(() => active() !== null && close(active()!)),
    },
    {
      id: "export",
      label: "File: Export PDF…",
      key: "⌥⌘E",
      run: run(exportPdf),
    },
    {
      id: "preview",
      label: "View: Toggle Page Preview",
      key: "⇧⌘V",
      run: togglePreview,
    },
    {
      id: "sidebar",
      label: "View: Toggle Sidebar",
      key: "⌘B",
      run: () => setSidebar((v) => !v),
    },
    { id: "theme", label: "View: Toggle Light / Dark", run: toggleTheme },
    { id: "find", label: "Edit: Find", key: "⌘F", run: find },
    {
      id: "outline",
      label: "View: Outline",
      run: () => {
        setSideView("outline");
        setSidebar(true);
      },
    },
  ];
  function dispatch(id: string) {
    if (id === "quit") {
      run(quit)();
      return;
    }
    if (id === "palette") {
      setQuery("");
      setPalette(true);
      return;
    }
    allCommands.find((c) => c.id === id)?.run();
  }
  function loadEditor() {
    const doc = current();
    if (!view || !doc || loaded === doc.id) return;
    if (loaded !== null) {
      states.set(loaded, view.state);
      scrolls.set(loaded, view.scrollDOM.scrollTop);
    }
    const id = doc.id;
    view.setState(states.get(id) ?? stateFor(doc, (text) => changed(id, text)));
    loaded = id;
    view.scrollDOM.scrollTop = scrolls.get(id) ?? 0;
    setAnalysis(null);
    setPreview(null);
    setExternal(false);
    setPreviewError("");
    void sync(id)
      .then(() => call<Analysis>("analyze_document", { id }))
      .then((a) => applyAnalysis(a, id))
      .catch(fail);
    if (previewOpen()) void updatePreview();
    queueMicrotask(() => view?.focus());
  }
  createEffect(() => {
    active();
    loadEditor();
  });
  createEffect(() => {
    document.documentElement.dataset.theme = dark() ? "dark" : "light";
  });
  onMount(() => {
    view = new EditorView({
      parent: host,
      state: stateFor(
        {
          id: 0,
          path: null,
          text: "",
          revision: 0,
          template: "manuscript",
          dirty: false,
        },
        () => {},
      ),
    });
    loadEditor();
    const unlisten = commands(dispatch).catch((e) => {
      fail(e);
      return () => {};
    });
    onCleanup(() => void unlisten.then((f) => f()));
    const key = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      const k = e.key.toLowerCase();
      let id = "";
      if (k === "n") id = "new";
      else if (k === "o") id = e.shiftKey ? "folder" : "open";
      else if (k === "s") id = e.shiftKey ? "save-as" : "save";
      else if (k === "w") id = "close";
      else if (k === "p" && e.shiftKey) id = "palette";
      else if (k === "v" && e.shiftKey) id = "preview";
      else if (k === "b") id = "sidebar";
      if (id) {
        e.preventDefault();
        dispatch(id);
      }
    };
    window.addEventListener("keydown", key);
    const theme = () => {
      if (!manualTheme) setDark(system.matches);
    };
    system.addEventListener("change", theme);
    const poll = async () => {
      if (checking || disposed) return;
      const d = current();
      if (!d?.path) return;
      checking = true;
      try {
        const changed = await call<boolean>("external_change", { id: d.id });
        if (active() === d.id) setExternal(changed);
      } catch {
        /* A tab may close during the check. */
      } finally {
        checking = false;
      }
    };
    const interval = setInterval(() => void poll(), 2000);
    window.addEventListener("focus", poll);
    onCleanup(() => {
      disposed = true;
      clearInterval(interval);
      clearTimeout(refreshTimer);
      window.removeEventListener("keydown", key);
      window.removeEventListener("focus", poll);
      system.removeEventListener("change", theme);
      view?.destroy();
    });
  });
  return (
    <main class="workbench">
      <div class="workbench-body">
        <nav class="activity-bar" aria-label="Activity">
          <Action
            label="Explorer"
            icon="files"
            active={sidebar() && sideView() === "explorer"}
            onClick={() => {
              setSidebar(true);
              setSideView("explorer");
            }}
          />
          <Action label="Find in file" icon="search" onClick={find} />
          <Action
            label="Outline"
            icon="list-tree"
            active={sidebar() && sideView() === "outline"}
            onClick={() => {
              setSidebar(true);
              setSideView("outline");
            }}
          />
          <div class="activity-spacer" />
          <Action
            label="Command Palette"
            icon="terminal"
            onClick={() => dispatch("palette")}
          />
          <Action
            label="Toggle Light / Dark"
            icon="color-mode"
            onClick={toggleTheme}
          />
        </nav>
        <Split class="workbench-split" initialSizes={[0.19, 0.81]}>
          <Show when={sidebar()}>
            <Split.Panel
              minSize="170px"
              maxSize="420px"
              initialSize="230px"
              class="sidebar"
            >
              <header>
                {sideView() === "outline" ? "OUTLINE" : "EXPLORER"}
                <Action
                  label="Open Folder"
                  icon="folder-opened"
                  onClick={run(openFolder)}
                />
              </header>
              <Show
                when={sideView() === "explorer"}
                fallback={
                  <div class="outline">
                    <For each={analysis()?.outline}>
                      {(item) => (
                        <button onClick={() => jump(item.source.bytes.start)}>
                          <Icon
                            name={
                              item.kind === "scene"
                                ? "symbol-event"
                                : "symbol-namespace"
                            }
                          />
                          <span>{item.text}</span>
                        </button>
                      )}
                    </For>
                    <Show when={!analysis()?.outline.length}>
                      <p class="muted">Scenes and headings appear here.</p>
                    </Show>
                  </div>
                }
              >
                <Show
                  when={folder()}
                  fallback={
                    <div class="sidebar-welcome">
                      <p>Open a folder to explore your story files.</p>
                      <button class="primary" onClick={run(openFolder)}>
                        Open Folder
                      </button>
                    </div>
                  }
                >
                  <div class="folder-title" title={folder()}>
                    {folder().split("/").pop()}
                  </div>
                  <For each={files()}>
                    {(file) => (
                      <button
                        class={`file ${current()?.path === file.path ? "selected" : ""}`}
                        title={file.path}
                        onClick={run(() => open(file.path))}
                      >
                        <Icon name="file-text" />
                        <span>{file.name}</span>
                      </button>
                    )}
                  </For>
                  <Show when={!files().length}>
                    <p class="muted">
                      No .md or .fountain files in this folder.
                    </p>
                  </Show>
                </Show>
              </Show>
            </Split.Panel>
            <Split.Handle class="split-handle" aria-label="Resize sidebar" />
          </Show>
          <Split.Panel minSize="440px" class="main-area">
            <header class="editor-title">
              <div class="tabs" role="tablist" aria-label="Open stories">
                <For each={tabs()}>
                  {(doc) => (
                    <div class={`tab ${active() === doc.id ? "selected" : ""}`}>
                      <button
                        role="tab"
                        aria-selected={active() === doc.id}
                        onClick={() => select(doc.id)}
                        title={doc.path ?? "Unsaved file"}
                      >
                        <Icon name="file-text" />
                        {nameOf(doc)}
                        <span
                          class="dirty"
                          aria-label={doc.dirty ? "Unsaved changes" : "Saved"}
                        >
                          {doc.dirty ? "●" : ""}
                        </span>
                      </button>
                      <button
                        class="tab-close"
                        aria-label={`Close ${nameOf(doc)}`}
                        onClick={run(() => close(doc.id))}
                      >
                        <Icon name="close" />
                      </button>
                    </div>
                  )}
                </For>
              </div>
              <Show when={current()}>
                {(doc) => (
                  <div class="editor-actions">
                    <TemplatePicker
                      value={doc().template}
                      onChange={(runTemplate) =>
                        run(() => changeTemplate(runTemplate))()
                      }
                    />
                    <Action
                      label="Toggle page preview"
                      icon="open-preview"
                      active={previewOpen()}
                      onClick={togglePreview}
                    />
                    <Action
                      label="Export PDF"
                      icon="export"
                      onClick={run(exportPdf)}
                    />
                  </div>
                )}
              </Show>
            </header>
            <Show when={error()}>
              <div class="message error" role="alert">
                <span>{error()}</span>
                <Action
                  label="Dismiss error"
                  icon="close"
                  onClick={() => setError("")}
                />
              </div>
            </Show>
            <Show when={external()}>
              <div class="message">
                <span>
                  This file changed outside River. Reload it, or Save As to keep
                  your edits.
                </span>
                <button onClick={run(reload)}>Reload</button>
                <button onClick={run(() => save(true))}>Save As…</button>
              </div>
            </Show>
            <div class="content">
              <Split class="editor-split" initialSizes={[0.5, 0.5]}>
                <Split.Panel minSize="300px" class="editor-pane">
                  <Show
                    when={current()}
                    fallback={
                      <section class="welcome">
                        <h1>River Writer</h1>
                        <p>Your story starts with a few words.</p>
                        <button onClick={run(newFile)}>
                          New File <kbd>⌘N</kbd>
                        </button>
                        <button onClick={run(() => open())}>
                          Open File… <kbd>⌘O</kbd>
                        </button>
                        <button onClick={run(openFolder)}>
                          Open Folder… <kbd>⇧⌘O</kbd>
                        </button>
                      </section>
                    }
                  >
                    {null}
                  </Show>
                  <div
                    ref={host}
                    class={`editor-host ${current() ? "" : "hidden"} ${current()?.template === "manuscript" ? "manuscript" : "script"}`}
                  />
                </Split.Panel>
                <Show when={previewOpen() && current()}>
                  <Split.Handle
                    class="split-handle"
                    aria-label="Resize page preview"
                  />
                  <Split.Panel minSize="240px" class="preview-pane">
                    <PagePreview
                      id={active()!}
                      preview={preview()}
                      busy={previewBusy()}
                      error={previewError()}
                      onClose={togglePreview}
                      onExport={run(exportPdf)}
                    />
                  </Split.Panel>
                </Show>
              </Split>
            </div>
            <Show when={analysis()?.diagnostics.length}>
              <div class="problems" aria-label="Problems">
                <For each={analysis()?.diagnostics}>
                  {(d) => (
                    <button onClick={() => jump(d.source.bytes.start)}>
                      <Icon
                        name={d.severity === "Error" ? "error" : "warning"}
                      />
                      {d.message}
                    </button>
                  )}
                </For>
              </div>
            </Show>
          </Split.Panel>
        </Split>
      </div>
      <footer class="status-bar">
        <span>{notice() || "River Writer"}</span>
        <div class="status-spacer" />
        <Show when={current()}>
          <span>{analysis()?.words ?? "—"} words</span>
          <span>{analysis()?.scenes ?? "—"} scenes</span>
          <span>UTF-8</span>
          <span>
            {current()?.path?.endsWith(".fountain") ? "Fountain" : "Markdown"}
          </span>
        </Show>
      </footer>
      <Palette
        open={palette()}
        onClose={() => setPalette(false)}
        query={query()}
        setQuery={setQuery}
        items={allCommands.filter((c) =>
          c.label.toLowerCase().includes(query().toLowerCase()),
        )}
      />
    </main>
  );
}
render(() => <App />, document.getElementById("root")!);
