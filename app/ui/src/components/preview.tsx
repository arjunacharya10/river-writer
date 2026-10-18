import {
  For,
  Show,
  createSignal,
  createEffect,
  onCleanup,
  onMount,
} from "solid-js";
import { call } from "../bridge";
import type { Preview } from "../types";
import { Action } from "./controls";
function Paper(props: {
  id: number;
  token: number;
  page: number;
  onError: (e: unknown) => void;
}) {
  let host!: HTMLDivElement;
  const [url, setUrl] = createSignal("");
  let disposed = false,
    requested = false,
    objectUrl = "";
  onMount(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          if (!requested) {
            requested = true;
            void call<ArrayBuffer>("render_page", {
              id: props.id,
              token: props.token,
              page: props.page,
            })
              .then((bytes) => {
                if (disposed) return;
                objectUrl = URL.createObjectURL(
                  new Blob([bytes], { type: "image/png" }),
                );
                setUrl(objectUrl);
              })
              .catch((e) => {
                if (!disposed) props.onError(e);
              });
          }
        }
      },
      { root: host.closest(".preview-scroll"), rootMargin: "300px" },
    );
    observer.observe(host);
    onCleanup(() => observer.disconnect());
  });
  onCleanup(() => {
    disposed = true;
    if (objectUrl) URL.revokeObjectURL(objectUrl);
  });
  return (
    <div ref={host} class="paper" data-page={props.page + 1}>
      <Show
        when={url()}
        fallback={<span class="paper-loading">Rendering page…</span>}
      >
        <img src={url()} alt={`Page ${props.page + 1}`} draggable={false} />
      </Show>
    </div>
  );
}
export function PagePreview(props: {
  id: number;
  preview: Preview | null;
  busy: boolean;
  error: string;
  onClose: () => void;
  onExport: () => void;
}) {
  const [page, setPage] = createSignal(1),
    [renderError, setRenderError] = createSignal("");
  let scroller!: HTMLDivElement;
  function visiblePage() {
    if (!scroller) return;
    const threshold =
      scroller.getBoundingClientRect().top +
      Math.min(100, scroller.clientHeight / 4);
    for (const paper of scroller.querySelectorAll<HTMLElement>("[data-page]")) {
      if (paper.getBoundingClientRect().bottom > threshold) {
        setPage(Number(paper.dataset.page));
        break;
      }
    }
  }
  createEffect(() => {
    props.preview?.token;
    setRenderError("");
    queueMicrotask(visiblePage);
  });
  function jump(n: number) {
    scroller
      .querySelector(`[data-page="${n}"]`)
      ?.scrollIntoView({ block: "start", behavior: "smooth" });
  }
  return (
    <section class="page-preview" aria-label="Page preview">
      <header class="preview-toolbar">
        <span>
          {props.busy ? "Updating…" : `${props.preview?.pages ?? 0} pages`}
        </span>
        <div class="page-navigation">
          <Action
            label="Previous page"
            icon="chevron-up"
            onClick={() => jump(Math.max(1, page() - 1))}
          />
          <span aria-label="Current page">{page()}</span>
          <Action
            label="Next page"
            icon="chevron-down"
            onClick={() =>
              jump(Math.min(props.preview?.pages ?? 1, page() + 1))
            }
          />
        </div>
        <Action label="Export PDF" icon="export" onClick={props.onExport} />
        <Action label="Close preview" icon="close" onClick={props.onClose} />
      </header>
      <Show when={props.error || renderError()}>
        <div class="preview-error" role="alert">
          {props.error || renderError()}
        </div>
      </Show>
      <div class="preview-scroll" ref={scroller} onScroll={visiblePage}>
        <Show when={props.preview} keyed>
          {(preview) => (
            <div data-token={preview.token}>
              <For each={Array.from({ length: preview.pages }, (_, i) => i)}>
                {(index) => (
                  <Paper
                    id={props.id}
                    token={preview.token}
                    page={index}
                    onError={(e) => {
                      if (!String(e).includes("expired"))
                        setRenderError(String(e));
                    }}
                  />
                )}
              </For>
            </div>
          )}
        </Show>
      </div>
    </section>
  );
}
