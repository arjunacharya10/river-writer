import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
// The browser test injects an IPC adapter; packaged builds always use Tauri.
declare global {
  interface Window {
    __RIVER_TEST__?: {
      invoke: <T>(name: string, args?: Record<string, unknown>) => Promise<T>;
    };
  }
}
export function call<T>(
  name: string,
  args?: Record<string, unknown>,
): Promise<T> {
  return (
    (import.meta.env.DEV
      ? window.__RIVER_TEST__?.invoke<T>(name, args)
      : undefined) ?? invoke<T>(name, args)
  );
}
export async function commands(handler: (id: string) => void) {
  if (import.meta.env.DEV && window.__RIVER_TEST__) return () => {};
  return listen<string>("river:command", (e) => handler(e.payload));
}
/** Files macOS asked River to open; the backend queues them until the UI asks. */
export async function openedFiles(handler: () => void) {
  if (import.meta.env.DEV && window.__RIVER_TEST__) return () => {};
  return listen("river:open-files", () => handler());
}
