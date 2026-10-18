import { render } from "solid-js/web";
import {
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
import { call, commands, openedFiles } from "./bridge";
import { stateFor, semantic } from "./editor";
import { writingContext } from "./writing";
import { Palette, Split, SplitVisibility } from "./components/controls";
import { ActivityBar } from "./components/activity-bar";
import { SidebarTitle, ExplorerView, OutlineView } from "./components/sidebar";
import { EditorTitle, EmptyEditor } from "./components/editor-title";
import {
  RecoveryNotice,
  ErrorBanner,
  RestoreBanner,
  ExternalChangeBanner,
} from "./components/banners";
import { GitDiffPane } from "./components/git-diff";
import { StatusBar, Problems } from "./components/status-bar";
import { createExtrasFlush } from "./features/extras-flush";
import { createGitWorkflow } from "./features/git-workflow";
import { shortcut, workbenchCommands } from "./features/commands";
import { ProjectWelcome } from "./components/welcome";
import { PagePreview } from "./components/preview";
import { TodosPanel } from "./components/todos";
import { SourceControl } from "./components/source-control";
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
// Order matters: later areas build on the base tokens and workbench chrome.
import "./styles/base.css";
import "./styles/workbench.css";
import "./styles/editor.css";
import "./styles/preview.css";
import "./styles/project-welcome.css";
import "./styles/extras.css";
import "./styles/git.css";
function App() {
  /** Bring open tabs and extras in line with files Git just rewrote on disk. */
  async function afterGitWrite(root: string, paths: string[]) {
    const changed = new Set(paths.map((p) => `${root}/${p}`));
    for (const doc of tabs()) {
      if (!doc.path || !changed.has(doc.path)) continue;
      cancelAutosave(doc.id);
      await queues.get(doc.id)?.catch(() => {});
      try {
        const d = await call<Story>("reload_document", { id: doc.id });
        patch(d.id, d);
        sent.set(d.id, d.revision);
        states.delete(d.id);
        if (active() === d.id) {
          loaded = null;
          loadEditor();
        }
      } catch {
        // The restore removed this file; its saved text is in Git.
        await call("close_document", { id: doc.id });
        const remaining = tabs().filter((d) => d.id !== doc.id);
        states.delete(doc.id);
        scrolls.delete(doc.id);
        sent.delete(doc.id);
        queues.delete(doc.id);
        setTabs(remaining);
        if (active() === doc.id) {
          loaded = null;
          setActive(remaining.at(-1)?.id ?? null);
        }
      }
    }
    setExternal(false);
    setExtrasEpoch((n) => n + 1);
    git.bump();
    if (folder())
      setFiles(await call<FileEntry[]>("refresh_folder", { path: folder() }));
    if (previewOpen()) void updatePreview();
  }
  async function prepareGitWrite(root: string) {
    await beforeCommit();
    const unsaved = tabs().find(
      (d) => d.dirty && d.path?.startsWith(root + "/"),
    );
    if (unsaved)
      throw Error(
        `${nameOf(unsaved)} has changes River could not save. Save or close it before restoring.`,
      );
  }
  async function beforeCommit() {
    await flushExtras();
    for (const doc of tabs()) if (doc.path) await autosave(doc.id);
  }
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
    if ((editor && !sidePaneOpen()) || (!editor && !sidebar())) return;
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
  const { flushExtras, registerExtrasFlush } = createExtrasFlush(fail);
  const [extrasEpoch, setExtrasEpoch] = createSignal(0);
  const git = createGitWorkflow({
    folder,
    prepare: prepareGitWrite,
    afterWrite: afterGitWrite,
    makeRoom: () => {
      if (previewOpen()) togglePreview();
    },
    notify: setNotice,
  });
  /** Preview and Git diff share one right-hand pane. */
  const sidePaneOpen = () => !!((previewOpen() && current()) || git.diff());
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
  /** Explorer's New File: create an empty story in the project and open it. */
  const [naming, setNaming] = createSignal(false);
  async function createFile(name: string) {
    const path = await call<string>("create_file", { root: folder(), name });
    setNaming(false);
    setFiles(await call<FileEntry[]>("refresh_folder", { path: folder() }));
    await open(path);
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
  let discardPending = false;
  async function confirmDiscard(name: string): Promise<boolean> {
    if (discardPending) return false;
    discardPending = true;
    try {
      return await call<boolean>("confirm_discard", { name });
    } finally {
      discardPending = false;
    }
  }
  async function mayLeave() {
    await flushExtras();
    for (const id of autosaveTimers.keys()) cancelAutosave(id);
    for (const d of tabs()) {
      if (d.dirty && !(await confirmDiscard(nameOf(d)))) {
        for (const doc of tabs()) if (doc.dirty) scheduleAutosave(doc.id);
        return false;
      }
    }
    return true;
  }
  async function clearWorkspace() {
    for (const id of autosaveTimers.keys()) cancelAutosave(id);
    git.setDiff(null);
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
        ["outline", "todos", "scratchpad", "source-control"].includes(
          saved.sideView,
        )
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
    // Saving a new .fountain file changes syntax, without changing its text or undo history.
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
    if (closing.has(id)) return;
    closing.add(id);
    try {
      await flushExtras();
      const doc = tabs().find((d) => d.id === id);
      if (!doc) return;
      cancelAutosave(id);
      if (doc.dirty && !(await confirmDiscard(nameOf(doc)))) {
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
        if (d.dirty && !(await confirmDiscard(nameOf(d)))) return;
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
    if (doc.dirty && !(await confirmDiscard(nameOf(doc)))) return;
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
  function jump(at: number) {
    if (!current() || !view) return;
    const pos = Math.min(at, view.state.doc.length);
    view.dispatch({
      selection: { anchor: pos },
      effects: EditorView.scrollIntoView(pos, { y: "center" }),
    });
    view.focus();
  }
  function togglePreview() {
    git.setDiff(null);
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
  const showView = (view: string) => {
    setSidebar(true);
    setSideView(view);
  };
  const allCommands = workbenchCommands({
    run,
    showView,
    newProject: () => loadProject(true),
    openProject: openFolder,
    closeProject,
    newFile,
    open: () => open(),
    save: () => save(),
    saveAs: () => save(true),
    closeTab: () => active() !== null && close(active()!),
    exportPdf,
    togglePreview,
    toggleSidebar: () => setSidebar((v) => !v),
    toggleTheme,
    find,
  });
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
  // Status messages are transient: clear each one after a few seconds.
  createEffect(
    on(notice, (text) => {
      if (!text) return;
      const timer = setTimeout(() => {
        if (notice() === text) setNotice("");
      }, 5000);
      onCleanup(() => clearTimeout(timer));
    }),
  );
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
    // Open Finder/Dock requests one at a time, after any project load finishes.
    let opening = Promise.resolve();
    const openRequested = () => {
      opening = opening
        .then(async () => {
          const paths = await call<string[]>("take_opened_files");
          for (const path of paths) await open(path);
        })
        .catch(fail);
    };
    const stopOpened = openedFiles(openRequested).catch(() => () => {});
    onCleanup(() => void stopOpened.then((f) => f()));
    if (!window.__RIVER_TEST__) openRequested();
    const key = (e: KeyboardEvent) => {
      const id = shortcut(e);
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
  return (
    <>
      <RecoveryNotice
        drafts={recoveries()}
        disabled={recovering() || projectBusy()}
        onRecover={(key) => void recoverDraft(key)}
      />
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
          <ActivityBar
            view={sideView()}
            sidebar={sidebar()}
            onShow={showView}
          />
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
              <SidebarTitle
                view={sideView()}
                onNewFile={
                  sideView() === "explorer" && folder()
                    ? () => setNaming(true)
                    : undefined
                }
              />
              <Show when={folder()} keyed>
                {(root) => (
                  <SourceControl
                    root={root}
                    active={sidebar() && sideView() === "source-control"}
                    beforeCommit={beforeCommit}
                    onReview={git.show}
                    refreshKey={git.epoch()}
                    onBranch={git.setBranch}
                  />
                )}
              </Show>
              <Show when={sideView() === "source-control" && !folder()}>
                <p class="muted">Open a project folder to use Git.</p>
              </Show>
              {/* Keyed on an epoch so extras remount (and reload) after Git rewrites files. */}
              <Show
                when={
                  extrasRoot() ? `${extrasEpoch()}\u0000${extrasRoot()}` : ""
                }
                keyed
              >
                {(key) => {
                  const root = key.slice(key.indexOf("\u0000") + 1);
                  return (
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
                  );
                }}
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
              <Show when={sideView() === "outline"}>
                <OutlineView items={analysis()?.outline} onJump={jump} />
              </Show>
              <Show when={sideView() === "explorer"}>
                <ExplorerView
                  folder={folder()}
                  files={files()}
                  currentPath={current()?.path}
                  onOpen={(path) => run(() => open(path))()}
                  onOpenProject={run(openFolder)}
                  naming={naming()}
                  onCreate={createFile}
                  onCancelCreate={() => setNaming(false)}
                />
              </Show>
            </Split.Panel>
            <Split.Handle
              class="split-handle"
              aria-label="Resize sidebar"
              style={{ display: sidebar() ? undefined : "none" }}
            />
            <Split.Panel
              minSize={sidePaneOpen() ? "444px" : "220px"}
              class="main-area"
              style={{
                "min-width": sidePaneOpen() ? "444px" : "220px",
              }}
            >
              <EditorTitle
                tabs={tabs()}
                active={active()}
                current={current()}
                onSelect={select}
                onClose={(id) => run(() => close(id))()}
                onTemplate={(template) => run(() => changeTemplate(template))()}
                previewOpen={previewOpen()}
                onPreview={togglePreview}
                onExport={run(exportPdf)}
              />
              <ErrorBanner error={error()} onDismiss={() => setError("")} />
              <RestoreBanner
                label={git.undo()?.label}
                busy={git.busy()}
                onUndo={run(git.undoRestore)}
                onDismiss={() => git.setUndo(null)}
              />
              <ExternalChangeBanner
                show={external()}
                onReload={run(reload)}
                onSaveAs={run(() => save(true))}
              />
              <div class="content">
                <Split
                  class="editor-split"
                  initialSizes={[0.5, 0.5]}
                  onSizesChange={(sizes) => rememberSizes(sizes, true)}
                >
                  <Split.Panel minSize="220px" class="editor-pane">
                    <Show when={!current()}>
                      <EmptyEditor
                        onNew={run(newFile)}
                        onOpen={run(() => open())}
                        onOpenProject={run(openFolder)}
                      />
                    </Show>
                    <div
                      ref={host}
                      class={`editor-host ${current() ? "" : "hidden"} ${current()?.template === "manuscript" ? "manuscript" : "script"}`}
                    />
                  </Split.Panel>
                  <Split.Handle
                    style={{
                      display: sidePaneOpen() ? undefined : "none",
                    }}
                    class="split-handle"
                    aria-label="Resize side pane"
                  />
                  <Split.Panel
                    minSize="220px"
                    class="preview-pane"
                    aria-hidden={!sidePaneOpen()}
                    inert={!sidePaneOpen()}
                  >
                    <GitDiffPane git={git} run={run} />
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
                    open={!!sidePaneOpen()}
                    panel={1}
                    defaultSizes={editorSizes()}
                    restoreKey={layoutVersion()}
                  />
                </Split>
              </div>
              <Problems items={analysis()?.diagnostics} onJump={jump} />
            </Split.Panel>
            <SplitVisibility
              open={sidebar()}
              panel={0}
              defaultSizes={sidebarSizes()}
              restoreKey={layoutVersion()}
            />
          </Split>
        </div>
        <StatusBar
          branch={folder() ? git.branch() : ""}
          root={folder()}
          switchBranch={git.switchBranch}
          notice={notice()}
          story={
            current()
              ? {
                  analysis: analysis(),
                }
              : null
          }
        />
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
