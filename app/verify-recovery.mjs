import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { once } from "node:events";
import assert from "node:assert/strict";
const root = fileURLToPath(new URL("../", import.meta.url));
const folder = await mkdtemp(`${tmpdir()}/river-recovery-ui-`);
const source = `${folder}/Script.fountain`;
const journals = `${folder}/.test-config/recovery`;
await writeFile(source, "Original story");
let backend,
  serial = 0;
const pending = new Map();
function start() {
  backend = spawn(`${root}/target/debug/examples/workbench_host`, [folder], {
    stdio: ["pipe", "pipe", "inherit"],
  });
  createInterface({ input: backend.stdout }).on("line", (line) => {
    const reply = JSON.parse(line),
      request = pending.get(reply.request);
    pending.delete(reply.request);
    reply.error
      ? request.reject(Error(reply.error))
      : request.resolve(reply.value);
  });
}
function invoke(name, args = {}) {
  return new Promise((resolve, reject) => {
    const request = ++serial;
    pending.set(request, { resolve, reject });
    backend.stdin.write(JSON.stringify({ request, name, args }) + "\n");
  });
}
async function crash() {
  const exited = once(backend, "exit");
  backend.kill("SIGKILL");
  await exited;
}
async function until(check) {
  for (let n = 0; n < 100; n++) {
    if (await check()) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw Error("Timed out waiting for durable recovery");
}
const browser = await chromium.launch({ headless: true, channel: "chrome" });
try {
  start();
  const doc = await invoke("open_document", { path: source });
  await invoke("sync_document", {
    id: doc.id,
    revision: 1,
    text: "Unsaved café 🎬",
    template: "screenplay",
  });
  const untitled = await invoke("new_document");
  await invoke("sync_document", {
    id: untitled.id,
    revision: 1,
    text: "My untitled draft",
    template: "manuscript",
  });
  // Kill the real service with unsaved buffers; no orderly close or journal cleanup.
  await crash();
  await writeFile(source, "Newer external edit");
  start();
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.exposeBinding("riverInvoke", (_, name, args) =>
    invoke(name, args),
  );
  await page.addInitScript(() => {
    window.__RIVER_TEST__ = {
      invoke: (name, args) => window.riverInvoke(name, args),
    };
  });
  await page.goto("http://127.0.0.1:1421");
  await page
    .getByRole("button", { name: "Recover Script.fountain", exact: true })
    .click();
  await page.locator(".cm-content").filter({ visible: true }).waitFor();
  await until(async () =>
    (await page.locator(".cm-content").innerText()).includes("Unsaved café 🎬"),
  );
  assert.equal(await readFile(source, "utf8"), "Newer external edit");
  await page
    .getByRole("button", { name: "Recover Untitled", exact: true })
    .click();
  await until(async () =>
    (await page.locator(".cm-content").innerText()).includes(
      "My untitled draft",
    ),
  );
  assert.equal(await page.getByLabel("Recovered drafts").count(), 0);
  await page.locator(".cm-content").click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.insertText(" continued");
  await until(async () => {
    const files = await readdir(journals);
    const drafts = await Promise.all(
      files
        .filter((f) => f.endsWith(".json"))
        .map(async (f) =>
          JSON.parse(await readFile(`${journals}/${f}`, "utf8")),
        ),
    );
    return drafts.some((d) => d.text === "My untitled draft continued");
  });
  await page.keyboard.press("ControlOrMeta+s");
  await until(
    async () =>
      (await readdir(journals)).filter((f) => f.endsWith(".json")).length === 1,
  );
  assert.equal(
    await readFile(`${folder}/Saved.md`, "utf8"),
    "My untitled draft continued",
  );
  assert.equal(await readFile(source, "utf8"), "Newer external edit");
  assert.deepEqual(errors, []);
  console.log(
    "PASS: abrupt service termination, named and untitled recovery, external edit preserved, continued writing journaled, saving clears recovery.",
  );
} finally {
  await browser.close();
  if (backend && backend.exitCode === null && backend.signalCode === null)
    await crash();
  await rm(folder, { recursive: true, force: true });
}
