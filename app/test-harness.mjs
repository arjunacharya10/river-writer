// Shared setup for the browser checks: the real Rust service (workbench_host)
// behind a page that talks to it in place of Tauri IPC.
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { once } from "node:events";
import { fileURLToPath } from "node:url";

export const root = fileURLToPath(new URL("../", import.meta.url));

/** Start the service for `folder`; call again after `crash()` to restart. */
export function startBackend(folder, env = {}) {
  const child = spawn(
    `${root}/target/debug/examples/workbench_host`,
    [folder],
    {
      stdio: ["pipe", "pipe", "inherit"],
      env: { ...process.env, ...env },
    },
  );
  const pending = new Map();
  const calls = [];
  let serial = 0;
  createInterface({ input: child.stdout }).on("line", (line) => {
    const reply = JSON.parse(line),
      request = pending.get(reply.request);
    pending.delete(reply.request);
    reply.error
      ? request.reject(Error(reply.error))
      : request.resolve(reply.value);
  });
  child.on("exit", (code) => {
    for (const request of pending.values())
      request.reject(Error(`Backend exited ${code}`));
  });
  const invoke = (name, args = {}) =>
    new Promise((resolve, reject) => {
      calls.push({ name, id: args.id, revision: args.revision });
      const request = ++serial;
      pending.set(request, { resolve, reject });
      child.stdin.write(JSON.stringify({ request, name, args }) + "\n");
    });
  return {
    invoke,
    calls,
    /** Kill without any orderly shutdown, as a crash would. */
    async crash() {
      if (child.exitCode !== null || child.signalCode !== null) return;
      const exited = once(child, "exit");
      child.kill("SIGKILL");
      await exited;
    },
    stop() {
      child.stdin.end();
    },
  };
}

/** A page wired to `invoke`. Set `window.__cancelNext = "<command>"` in the page
 * to make the next such call behave like a cancelled native dialog. */
export async function openPage(browser, invoke) {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 840 },
  });
  page.setDefaultTimeout(10000);
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("response", (r) => {
    if (r.status() >= 400) errors.push(`${r.status()} ${r.url()}`);
  });
  await page.exposeBinding("riverInvoke", (_, name, args) =>
    invoke(name, args),
  );
  await page.addInitScript(() => {
    window.__RIVER_TEST__ = {
      invoke: async (name, args) => {
        if (window.__cancelNext === name) {
          window.__cancelNext = null;
          return null;
        }
        const result = await window.riverInvoke(name, args);
        return name === "render_page" ? new Uint8Array(result).buffer : result;
      },
    };
  });
  await page.goto("http://127.0.0.1:1421/");
  return { page, errors };
}

export async function until(check, message = "Timed out waiting for the app") {
  for (let n = 0; n < 100; n++) {
    try {
      if (await check()) return;
    } catch {
      // Not ready yet (e.g. a file not written so far).
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw Error(message);
}
