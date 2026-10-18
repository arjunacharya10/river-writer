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
/** App-shell commands handled by Tauri itself rather than the shared table. */
const shell = new Set(["take_opened_files", "quit_confirmed"]);
/** Commands whose reply is raw bytes (an ArrayBuffer), not JSON. */
const binary = new Set(["render_page"]);
/** Call a workbench command by name; see `river_service::commands`. */
export function call<T>(
  name: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const test = import.meta.env.DEV
    ? window.__RIVER_TEST__?.invoke<T>(name, args)
    : undefined;
  if (test) return test;
  if (shell.has(name)) return invoke<T>(name, args);
  return invoke<T>(binary.has(name) ? "river_bytes" : "river", { name, args });
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
