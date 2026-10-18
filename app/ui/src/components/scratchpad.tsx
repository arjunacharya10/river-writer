import { Show, createEffect, createSignal, on, onCleanup } from "solid-js";
import { call } from "../bridge";
type Snapshot = { text: string; revision: string | null };
export function ScratchpadPanel(props: {
  root: string;
  active: boolean;
  onError: (e: unknown) => void;
  registerFlush: (flush: () => Promise<void>) => () => void;
}) {
  const root = props.root;
  const [text, setText] = createSignal(""),
    [ready, setReady] = createSignal(false),
    [error, setError] = createSignal("");
  const [resolving, setResolving] = createSignal(false);
  let latest: Snapshot | null = null,
    disposed = false,
    loading: Promise<void> | null = null;
  let queue: Promise<void> = Promise.resolve();
  let timer: ReturnType<typeof setTimeout> | undefined,
    journal: ReturnType<typeof setTimeout> | undefined;
  const fail = (e: unknown) => {
    if (disposed) props.onError(e);
    else setError(String(e));
  };
  function enqueue(task: () => Promise<void>) {
    const next = queue.catch(() => {}).then(task);
    queue = next;
    return next;
  }
  function load(force = false) {
    if (loading) return loading;
    if (!force && latest && text() !== latest.text) return Promise.resolve();
    loading = (async () => {
      try {
        await queue.catch(() => {});
        const value = await call<Snapshot>("load_scratchpad", { root });
        latest = value;
        if (!disposed) {
          setText(value.text);
          setReady(true);
          setError("");
        }
      } catch (e) {
        fail(e);
      } finally {
        loading = null;
      }
    })();
    return loading;
  }
  function protect() {
    if (!latest) return Promise.resolve();
    const value = text();
    return enqueue(() =>
      call<void>("protect_scratchpad", { root, text: value }),
    );
  }
  function save() {
    if (!latest || text() === latest.text) return Promise.resolve();
    const value = text();
    return enqueue(async () => {
      latest = await call<Snapshot>("save_scratchpad", {
        root,
        expected: latest!.revision,
        text: value,
      });
      if (!disposed) setError("");
    });
  }
  async function flush() {
    clearTimeout(timer);
    clearTimeout(journal);
    if (loading) await loading;
    await queue.catch(() => {});
    await save();
    await protect();
  }
  function changed(value: string) {
    setText(value);
    clearTimeout(timer);
    clearTimeout(journal);
    journal = setTimeout(() => void protect().catch(fail), 120);
    timer = setTimeout(() => void save().catch(fail), 1000);
  }
  async function keepAndReload() {
    setResolving(true);
    try {
      clearTimeout(timer);
      clearTimeout(journal);
      await queue.catch(() => {});
      const copied = await call<boolean>("copy_scratchpad", {
        root,
        text: text(),
      });
      if (!copied) return;
      await load(true);
      // The conflicted text is now safely in the chosen copy. Clear its journal only
      // after acknowledging the disk snapshot we have actually loaded.
      if (latest)
        await call("settle_scratchpad", { root, expected: latest.revision });
    } finally {
      setResolving(false);
    }
  }
  const unregister = props.registerFlush(flush);
  createEffect(
    on(
      () => props.active,
      (active) => {
        if (active) void load();
        else void flush().catch(fail);
      },
    ),
  );
  const focus = () => {
    if (props.active) void load();
  };
  window.addEventListener("focus", focus);
  onCleanup(() => {
    unregister();
    window.removeEventListener("focus", focus);
    void flush().catch(props.onError);
    disposed = true;
  });
  return (
    <section
      class="scratchpad-panel"
      aria-label="Scratchpad"
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
      <Show when={error()}>
        <div class="extras-error" role="alert">
          <span>{error()}</span>
          <Show
            when={ready()}
            fallback={<button onClick={() => void load()}>Retry</button>}
          >
            <button
              disabled={resolving()}
              onClick={() => void keepAndReload().catch(fail)}
            >
              Save a copy and reload
            </button>
          </Show>
        </div>
      </Show>
      <textarea
        aria-label="Scratchpad notes"
        placeholder="Notes, loose ideas, anything…"
        spellcheck
        disabled={!ready() || resolving()}
        value={text()}
        onInput={(event) => changed(event.currentTarget.value)}
        onBlur={() => {
          if (!resolving()) void flush().catch(fail);
        }}
      />
    </section>
  );
}
