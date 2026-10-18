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
}) {
  const [renderError, setRenderError] = createSignal("");
  createEffect(() => {
    props.preview?.token;
    setRenderError("");
  });
  return (
    <section class="page-preview" aria-label="Page preview">
      <header class="preview-toolbar">
        <span>
          {props.busy
            ? "Updating…"
            : `${props.preview?.pages ?? 0} ${props.preview?.pages === 1 ? "page" : "pages"}`}
        </span>
      </header>
      <Show when={props.error || renderError()}>
        <div class="preview-error" role="alert">
          {props.error || renderError()}
        </div>
      </Show>
      <div class="preview-scroll" tabindex="0" aria-label="Preview pages">
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
