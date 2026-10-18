import { render } from "solid-js/web";
import {
  For,
  Show,
  createSignal,
  createEffect,
  onMount,
  onCleanup,
  on,
} from "solid-js";
import { EditorView } from "@codemirror/view";
import type { EditorState } from "@codemirror/state";
import { openSearchPanel } from "@codemirror/search";
import { isolateHistory } from "@codemirror/commands";
import { call, commands } from "./bridge";
import { stateFor, semantic, byteOffsets } from "./editor";
import { writingContext } from "./writing";
import {
  Action,
  Icon,
  TemplatePicker,
  Palette,
  Split,
  SplitVisibility,
  type Command,
} from "./components/controls";
import { ProjectWelcome } from "./components/welcome";
import { PagePreview } from "./components/preview";
import { TodosPanel } from "./components/todos";
import { ScratchpadPanel } from "./components/scratchpad";
import {
  nameOf,
  type Story,
  type Analysis,
  type Preview,
  type FileEntry,
  type TemplateId,
  type Project,
  type RecentProject,
  type ProjectSession,
} from "./types";
import "@vscode/codicons/dist/codicon.css";
import "./style.css";
function App() {
  const extraFlushers = new Set<() => Promise<void>>();
  async function flushExtras() {
    await Promise.all([...extraFlushers].map((flush) => flush()));
  }
  function registerExtrasFlush(flush: () => Promise<void>) {
    extraFlushers.add(flush);
    return () => {
      queueMicrotask(() => {
        void flush().then(
          () => extraFlushers.delete(flush),
          (error) => {
            extraFlushers.delete(flush);
            fail(error);
          },
        );
      });
    };
  }
  type Version = { id: string; timestamp: number; bytes: number };
  const [historyOpen, setHistoryOpen] = createSignal(false);
  const [versions, setVersions] = createSignal<Version[]>([]);
  const [historyVersion, setHistoryVersion] = createSignal("");
  const [historyText, setHistoryText] = createSignal("");
  const [historyBusy, setHistoryBusy] = createSignal(false);
  const [historyMore, setHistoryMore] = createSignal(false);
  let historyRequest = 0;
  let historyRead = 0;
  const autosaveTimers = new Map<number, ReturnType<typeof setTimeout>>();
  const closing = new Set<number>();
  function cancelAutosave(id: number) {
    clearTimeout(autosaveTimers.get(id));
    autosaveTimers.delete(id);
  }
  function scheduleAutosave(id: number) {
    cancelAutosave(id);
    autosaveTimers.set(
      id,
      setTimeout(() => {
        autosaveTimers.delete(id);
        if (!quitting && !projectBusy() && !closing.has(id))
          void autosave(id).catch((e) =>
            fail(
              `Could not automatically save ${nameOf(tabs().find((d) => d.id === id) ?? ({ path: null } as Story))}: ${e}`,
            ),
          );
      }, 1000),
    );
  }
  async function autosave(id: number) {
    await sync(id);
    const doc = tabs().find((d) => d.id === id);
    if (!doc?.path || !doc.dirty || closing.has(id)) return;
    const task = (queues.get(id) ?? Promise.resolve())
      .catch(() => {})
      .then(() => {
        if (closing.has(id)) return null;
        return call<Story | null>("autosave_document", {
          id,
          revision: doc.revision,
        });
      });
    queues.set(id, task);
    const result = await task;
    if (!result) return;
    const local = tabs().find((d) => d.id === id);
    if (!local) return;
    patch(id, { dirty: local.revision !== result.revision });
    if (active() === id) setNotice("Saved automatically");
    if (historyOpen() && active() === id) await loadHistory();
  }
  async function loadHistory(older = false) {
    const doc = current();
    const ticket = ++historyRequest;
    if (!doc?.path) {
      setVersions([]);
      setHistoryBusy(false);
      setHistoryMore(false);
      return;
    }
    setHistoryBusy(true);
    try {
      const items = await call<Version[]>("local_history", {
        id: doc.id,
        before: older ? versions().at(-1)?.id : null,
      });
      if (ticket === historyRequest && active() === doc.id) {
        setVersions((previous) => (older ? [...previous, ...items] : items));
        setHistoryMore(items.length === 200);
      }
    } finally {
      if (ticket === historyRequest) setHistoryBusy(false);
    }
  }
  async function inspectVersion(version: string) {
    const doc = current();
    if (!doc) return;
    const ticket = ++historyRead;
    setHistoryVersion("");
    const text = await call<string>("history_text", { id: doc.id, version });
    if (ticket !== historyRead || active() !== doc.id) return;
    setHistoryText(text);
    setHistoryVersion(version);
  }
  async function restoreVersion() {
    const doc = current(),
      version = historyVersion();
    if (!doc || !version) return;
    await sync(doc.id);
    const text = await call<string>("prepare_restore", {
      id: doc.id,
      revision: doc.revision,
      version,
    });
    if (active() !== doc.id || current()?.revision !== doc.revision)
      throw Error("The story changed. Choose the version again.");
    if (view && view.state.doc.toString() !== text) {
      view.dispatch({
        annotations: isolateHistory.of("full"),
        changes: { from: 0, to: view.state.doc.length, insert: text },
      });
      view.focus();
    }
    setNotice("Version restored. Undo returns to your previous text.");
    await loadHistory();
  }
  const [recoveries, setRecoveries] = createSignal<
    { key: string; path: string | null }[]
  >([]);
  const [recovering, setRecovering] = createSignal(false);
  async function recoverDraft(key: string) {
    setRecovering(true);
    try {
      const doc = await call<Story>("recover_draft", { key });
      setWelcome(false);
      await add(doc);
      setRecoveries((drafts) => drafts.filter((draft) => draft.key !== key));
      setNotice("Recovered as an unsaved copy. Save to choose its location.");
    } catch (e) {
      fail(e);
    } finally {
      setRecovering(false);
    }
  }
  const [welcome, setWelcome] = createSignal(true),
    [recent, setRecent] = createSignal<RecentProject[]>([]),
    [projectBusy, setProjectBusy] = createSignal(false);
  let sessionTimer: ReturnType<typeof setTimeout> | undefined;
  let restoring = false;
  const [sidebarSizes, setSidebarSizes] = createSignal<[number, number]>([
      0.19, 0.81,
    ]),
    [editorSizes, setEditorSizes] = createSignal<[number, number]>([0.5, 0.5]),
    [layoutVersion, setLayoutVersion] = createSignal(0);
  function rememberSizes(sizes: number[], editor: boolean) {
    if (
      projectBusy() ||
      welcome() ||
      sizes.length !== 2 ||
      sizes.some((size) => !Number.isFinite(size) || size <= 0)
    )
      return;
    if ((editor && !previewOpen() && !historyOpen()) || (!editor && !sidebar()))
      return;
    const total = sizes[0] + sizes[1];
    (editor ? setEditorSizes : setSidebarSizes)([
      sizes[0] / total,
      sizes[1] / total,
    ]);
  }

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
    quitting = false,
    openIntent = 0;
  const states = new Map<number, EditorState>(),
    scrolls = new Map<number, number>(),
    sent = new Map<number, number>(),
    queues = new Map<number, Promise<unknown>>();
  const current = () => tabs().find((d) => d.id === active());
  const extrasRoot = () => {
    const path = current()?.path;
    return folder() || (path ? path.slice(0, path.lastIndexOf("/")) : "");
  };
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
      effects: [
        semantic.of({
          analysis: a,
          script: current()?.template !== "manuscript",
        }),
        writingContext.of({
          enabled:
            current()?.path?.toLowerCase().endsWith(".fountain") ?? false,
          analysis: a,
        }),
      ],
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
    scheduleAutosave(id);
  }
  function select(id: number) {
    openIntent++;
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
    await flushExtras();
    await add(await call<Story>("new_document"));
  }
  async function open(path?: string) {
    await flushExtras();
    // Selecting an already-open story is immediate; a delayed open response must
    // not steal focus back after the writer switches to another tab.
    const existing = path ? tabs().find((doc) => doc.path === path) : undefined;
    if (existing) {
      select(existing.id);
      return;
    }
    const intent = ++openIntent;
    const doc = await call<Story | null>("open_document", {
      path: path ?? null,
    });
    if (doc) {
      if (intent !== openIntent) {
        if (!tabs().some((tab) => tab.id === doc.id)) {
          setTabs((ds) => [...ds, doc]);
          sent.set(doc.id, doc.revision);
        }
        return;
      }
      setWelcome(false);
      await add(doc);
      view?.requestMeasure();
    }
  }
  async function refreshRecent() {
    setRecent(await call<RecentProject[]>("recent_projects"));
  }
  function sessionSnapshot(): ProjectSession {
    if (view && loaded !== null) {
      states.set(loaded, view.state);
      scrolls.set(loaded, view.scrollDOM.scrollTop);
    }
    const relative = (path: string | null) =>
      path?.startsWith(folder() + "/") ? path.slice(folder().length + 1) : null;
    const openFiles: string[] = [],
      cursors: Record<string, number> = {},
      positions: Record<string, number> = {};
    for (const doc of tabs()) {
      const name = relative(doc.path);
      if (name) {
        openFiles.push(name);
        cursors[name] = states.get(doc.id)?.selection.main.head ?? 0;
        positions[name] = scrolls.get(doc.id) ?? 0;
      }
    }
    return {
      openFiles,
      sidebarSizes: sidebarSizes(),
      editorSizes: editorSizes(),
      activeFile: relative(current()?.path ?? null),
      cursors,
      scrolls: positions,
      sidebar: sidebar(),
      sideView: sideView(),
      preview: previewOpen(),
      theme: manualTheme ? (dark() ? "dark" : "light") : null,
    };
  }
  async function rememberSession() {
    clearTimeout(sessionTimer);
    if (folder())
      await call("save_project_session", {
        path: folder(),
        session: sessionSnapshot(),
      });
  }
  async function mayLeave() {
    await flushExtras();
    for (const id of autosaveTimers.keys()) cancelAutosave(id);
    for (const d of tabs()) {
      if (
        d.dirty &&
        !(await call<boolean>("confirm_discard", { name: nameOf(d) }))
      ) {
        for (const doc of tabs()) if (doc.dirty) scheduleAutosave(doc.id);
        return false;
      }
    }
    return true;
  }
  async function clearWorkspace() {
    for (const id of autosaveTimers.keys()) cancelAutosave(id);
    setHistoryVersion("");
    setHistoryText("");
    setVersions([]);
    historyRequest++;
    historyRead++;
    clearTimeout(refreshTimer);
    previewRequest++;
    await Promise.all([...queues.values()].map((p) => p.catch(() => {})));
    await Promise.all(tabs().map((d) => call("close_document", { id: d.id })));
    loaded = null;
    setActive(null);
    setTabs([]);
    states.clear();
    scrolls.clear();
    sent.clear();
    queues.clear();
    setAnalysis(null);
    setPreview(null);
    setExternal(false);
    setNotice("");
  }
  async function closeProject() {
    if (projectBusy() || !(await mayLeave())) return;
    await rememberSession();
    await clearWorkspace();
    setFolder("");
    setWelcome(true);
    setPalette(false);
    setError("");
    await refreshRecent();
  }
  async function loadProject(create = false, path?: string) {
    if (projectBusy()) return;
    setProjectBusy(true);
    setError("");
    restoring = true;
    try {
      if (!(await mayLeave())) return;
      await rememberSession();
      const project = await call<Project | null>(
        create ? "create_project" : "open_project",
        create ? undefined : { path: path ?? null },
      );
      if (!project) return;
      await clearWorkspace();
      setFolder(project.path);
      setFiles(project.files);
      setWelcome(false);
      const saved = project.session;
      setSidebar(saved.sidebar);
      setSideView(
        ["outline", "todos", "scratchpad"].includes(saved.sideView)
          ? saved.sideView
          : "explorer",
      );
      setPreviewOpen(saved.preview);
      setSidebarSizes(saved.sidebarSizes);
      setEditorSizes(saved.editorSizes);
      setLayoutVersion((v) => v + 1);
      manualTheme = !!saved.theme;
      setDark(saved.theme ? saved.theme === "dark" : system.matches);
      const names = saved.openFiles.length
        ? saved.openFiles
        : project.files
            .slice(0, 1)
            .map((f) => f.path.slice(project.path.length + 1));
      for (const name of names) {
        const doc = await call<Story | null>("open_document", {
          path: project.path + "/" + name,
        });
        if (!doc) continue;
        await add(doc);
        const state = stateFor(doc, (text) => changed(doc.id, text));
        const anchor = Math.min(saved.cursors[name] ?? 0, state.doc.length);
        const restored = state.update({ selection: { anchor } }).state;
        states.set(doc.id, restored);
        scrolls.set(doc.id, saved.scrolls[name] ?? 0);
        if (view && loaded === doc.id) {
          view.setState(restored);
          view.scrollDOM.scrollTop = saved.scrolls[name] ?? 0;
        }
      }
      if (!tabs().length) await newFile();
      const selected =
        tabs().find((d) => d.path === project.path + "/" + saved.activeFile) ??
        tabs()[0];
      // Force the restored EditorState to replace the initial state made by add().
      loaded = null;
      setActive(selected.id);
      loadEditor();
      await refreshRecent();
      queueMicrotask(() => {
        view?.requestMeasure();
        if (saved.preview) void updatePreview();
      });
    } finally {
      restoring = false;
      setProjectBusy(false);
    }
  }
  async function openFolder() {
    await loadProject();
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
      template:
        local?.revision === result.revision
          ? result.template
          : (local?.template ?? result.template),
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
    await flushExtras();
    const doc = tabs().find((d) => d.id === id);
    if (!doc) return;
    cancelAutosave(id);
    closing.add(id);
    try {
      if (
        doc.dirty &&
        !(await call<boolean>("confirm_discard", { name: nameOf(doc) }))
      ) {
        scheduleAutosave(id);
        return;
      }
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
    } finally {
      closing.delete(id);
    }
  }
  async function quit() {
    if (quitting) return;
    quitting = true;
    for (const id of autosaveTimers.keys()) cancelAutosave(id);
    try {
      await flushExtras();
      for (const d of tabs()) {
        if (
          d.dirty &&
          !(await call<boolean>("confirm_discard", { name: nameOf(d) }))
        )
          return;
      }
      await rememberSession();
      await clearWorkspace();
      await call("quit_confirmed");
    } finally {
      quitting = false;
      for (const doc of tabs()) if (doc.dirty) scheduleAutosave(doc.id);
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
    setHistoryOpen(false);
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
    {
      id: "scratchpad",
      label: "View: Scratchpad",
      run: () => {
        setSideView("scratchpad");
        setSidebar(true);
      },
    },
    {
      id: "new-project",
      label: "Project: New Project…",
      key: "⇧⌘N",
      run: run(() => loadProject(true)),
    },
    {
      id: "close-project",
      label: "Project: Close Project",
      key: "⇧⌘W",
      run: run(closeProject),
    },
    { id: "new", label: "File: New File", key: "⌘N", run: run(newFile) },
    { id: "open", label: "File: Open…", key: "⌘O", run: run(() => open()) },
    {
      id: "folder",
      label: "Project: Open Project…",
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
      id: "todos",
      label: "View: Todos",
      run: () => {
        setSideView("todos");
        setSidebar(true);
      },
    },
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
    if (projectBusy()) return;
    if (welcome() && (id === "new" || id === "new-project")) {
      run(() => loadProject(true))();
      return;
    }
    if (welcome() && (id === "open" || id === "folder")) {
      run(() => loadProject())();
      return;
    }
    if (welcome() && id !== "quit") return;
    if (
      id === "save" &&
      document.activeElement?.closest(".todos-panel, .scratchpad-panel")
    ) {
      run(flushExtras)();
      return;
    }
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
    if (previewOpen() && !restoring) void updatePreview();
    if (!restoring) queueMicrotask(() => view?.focus());
  }
  createEffect(() => {
    active();
    loadEditor();
  });
  createEffect(() => {
    document.documentElement.dataset.theme = dark() ? "dark" : "light";
  });
  createEffect(() => {
    folder();
    tabs();
    active();
    sidebar();
    sideView();
    previewOpen();
    sidebarSizes();
    editorSizes();
    dark();
    welcome();
    projectBusy();
    clearTimeout(sessionTimer);
    if (folder() && !welcome() && !projectBusy())
      sessionTimer = setTimeout(() => void rememberSession().catch(fail), 600);
  });
  onMount(() => {
    void call<{ key: string; path: string | null }[]>("recovery_drafts")
      .then(setRecoveries)
      .catch(fail);
    void refreshRecent().catch(fail);
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
      if (k === "n") id = e.shiftKey ? "new-project" : "new";
      else if (k === "o") id = e.shiftKey ? "folder" : "open";
      else if (k === "s") id = e.shiftKey ? "save-as" : "save";
      else if (k === "w") id = e.shiftKey ? "close-project" : "close";
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
      clearTimeout(sessionTimer);
      for (const id of autosaveTimers.keys()) cancelAutosave(id);
      window.removeEventListener("keydown", key);
      window.removeEventListener("focus", poll);
      system.removeEventListener("change", theme);
      view?.destroy();
    });
  });
  createEffect(
    on([active, historyOpen], ([, open]) => {
      historyRead++;
      setHistoryVersion("");
      setHistoryText("");
      setVersions([]);
      setHistoryMore(false);
      if (open) void loadHistory().catch(fail);
    }),
  );
  return (
    <>
      <Show when={recoveries().length > 0}>
        <aside class="recovery-notice" aria-label="Recovered drafts">
          <span>River found writing from an interrupted session.</span>
          <For each={recoveries()}>
            {(draft) => (
              <button
                disabled={recovering() || projectBusy()}
                onClick={() => void recoverDraft(draft.key)}
              >
                Recover {draft.path?.split("/").at(-1) ?? "Untitled"}
              </button>
            )}
          </For>
        </aside>
      </Show>
      <Show when={welcome()}>
        <ProjectWelcome
          recent={recent()}
          busy={projectBusy()}
          error={error()}
          onNew={run(() => loadProject(true))}
          onOpen={(path) => run(() => loadProject(false, path))()}
          onDismiss={() => setError("")}
        />
      </Show>
      <main
        class="workbench"
        style={{ display: welcome() ? "none" : undefined }}
        inert={welcome() || projectBusy()}
        aria-hidden={welcome()}
      >
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
            <Action
              label="Scratchpad"
              icon="edit"
              active={sidebar() && sideView() === "scratchpad"}
              onClick={() => {
                setSidebar(true);
                setSideView("scratchpad");
              }}
            />
            <Action
              label="Todos"
              icon="checklist"
              active={sidebar() && sideView() === "todos"}
              onClick={() => {
                setSidebar(true);
                setSideView("todos");
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
          <Split
            class="workbench-split"
            initialSizes={[0.19, 0.81]}
            onSizesChange={(sizes) => rememberSizes(sizes, false)}
          >
            <Split.Panel
              aria-hidden={!sidebar()}
              inert={!sidebar()}
              minSize="170px"
              maxSize="420px"
              class="sidebar"
            >
              <header>
                {sideView() === "scratchpad"
                  ? "SCRATCHPAD"
                  : sideView() === "todos"
                    ? "TODOS"
                    : sideView() === "outline"
                      ? "OUTLINE"
                      : "EXPLORER"}
                <Action
                  label="Open Project"
                  icon="folder-opened"
                  onClick={run(openFolder)}
                />
              </header>
              <Show when={extrasRoot()} keyed>
                {(root) => (
                  <>
                    <ScratchpadPanel
                      root={root}
                      active={sidebar() && sideView() === "scratchpad"}
                      onError={fail}
                      registerFlush={registerExtrasFlush}
                    />
                    <TodosPanel
                      root={root}
                      active={sidebar() && sideView() === "todos"}
                      onError={fail}
                      registerFlush={registerExtrasFlush}
                    />
                  </>
                )}
              </Show>
              <Show
                when={
                  ["todos", "scratchpad"].includes(sideView()) && !extrasRoot()
                }
              >
                <p class="muted">
                  Open a project or save your story to keep notes and tasks
                  alongside it.
                </p>
              </Show>
              <Show when={!["todos", "scratchpad"].includes(sideView())}>
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
                          Open Project
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
              </Show>
            </Split.Panel>
            <Split.Handle
              class="split-handle"
              aria-label="Resize sidebar"
              style={{ display: sidebar() ? undefined : "none" }}
            />
            <Split.Panel
              minSize={
                (previewOpen() || historyOpen()) && current()
                  ? "444px"
                  : "220px"
              }
              class="main-area"
              style={{
                "min-width":
                  (previewOpen() || historyOpen()) && current()
                    ? "444px"
                    : "220px",
              }}
            >
              <header class="editor-title">
                <div class="tabs" role="tablist" aria-label="Open stories">
                  <For each={tabs()}>
                    {(doc) => (
                      <div
                        class={`tab ${active() === doc.id ? "selected" : ""}`}
                      >
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
                        label="Toggle local history"
                        icon="history"
                        active={historyOpen()}
                        onClick={() => {
                          if (!historyOpen() && previewOpen()) togglePreview();
                          setHistoryOpen(!historyOpen());
                        }}
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
                    This file changed outside River. Reload it, or Save As to
                    keep your edits.
                  </span>
                  <button onClick={run(reload)}>Reload</button>
                  <button onClick={run(() => save(true))}>Save As…</button>
                </div>
              </Show>
              <div class="content">
                <Split
                  class="editor-split"
                  initialSizes={[0.5, 0.5]}
                  onSizesChange={(sizes) => rememberSizes(sizes, true)}
                >
                  <Split.Panel minSize="220px" class="editor-pane">
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
                            Open Project… <kbd>⇧⌘O</kbd>
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
                  <Split.Handle
                    style={{
                      display:
                        (previewOpen() || historyOpen()) && current()
                          ? undefined
                          : "none",
                    }}
                    class="split-handle"
                    aria-label="Resize side pane"
                  />
                  <Split.Panel
                    minSize="220px"
                    class="preview-pane"
                    aria-hidden={
                      !((previewOpen() || historyOpen()) && current())
                    }
                    inert={!((previewOpen() || historyOpen()) && current())}
                  >
                    <Show when={historyOpen()}>
                      <section class="timeline" aria-label="Local history">
                        <header>
                          <strong>TIMELINE · LOCAL HISTORY</strong>
                          <Action
                            label="Close local history"
                            icon="close"
                            onClick={() => setHistoryOpen(false)}
                          />
                        </header>
                        <div class="timeline-body">
                          <div class="timeline-list">
                            <For each={versions()}>
                              {(version) => (
                                <button
                                  class={
                                    historyVersion() === version.id
                                      ? "selected"
                                      : ""
                                  }
                                  aria-pressed={historyVersion() === version.id}
                                  onClick={run(() =>
                                    inspectVersion(version.id),
                                  )}
                                >
                                  {new Date(
                                    version.timestamp * 1000,
                                  ).toLocaleString()}{" "}
                                  · {version.bytes} bytes
                                </button>
                              )}
                            </For>
                            <Show when={historyMore()}>
                              <button
                                disabled={historyBusy()}
                                onClick={run(() => loadHistory(true))}
                              >
                                Load older versions
                              </button>
                            </Show>
                            <Show when={!versions().length}>
                              <p class="muted">
                                {historyBusy()
                                  ? "Loading…"
                                  : current()?.path
                                    ? "Saved versions will appear here."
                                    : "Save this draft to start local history."}
                              </p>
                            </Show>
                          </div>
                          <div class="timeline-version">
                            <Show
                              when={historyVersion()}
                              fallback={
                                <p class="muted">
                                  Choose a version to read its text.
                                </p>
                              }
                            >
                              <button onClick={run(restoreVersion)}>
                                Restore this version
                              </button>
                              <pre aria-label="Version text">
                                {historyText()}
                              </pre>
                            </Show>
                          </div>
                        </div>
                      </section>
                    </Show>
                    <Show when={previewOpen() && current()}>
                      <PagePreview
                        id={active()!}
                        preview={preview()}
                        busy={previewBusy()}
                        error={previewError()}
                      />
                    </Show>
                  </Split.Panel>
                  <SplitVisibility
                    open={!!((previewOpen() || historyOpen()) && current())}
                    panel={1}
                    defaultSizes={editorSizes()}
                    restoreKey={layoutVersion()}
                  />
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
            <SplitVisibility
              open={sidebar()}
              panel={0}
              defaultSizes={sidebarSizes()}
              restoreKey={layoutVersion()}
            />
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
    </>
  );
}
render(() => <App />, document.getElementById("root")!);
