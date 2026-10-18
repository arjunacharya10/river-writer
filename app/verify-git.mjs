import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import {
  mkdtemp,
  writeFile,
  appendFile,
  readFile,
  rm,
  mkdir,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
const root = fileURLToPath(new URL("../", import.meta.url));
const folder = await mkdtemp(`${tmpdir()}/river-git-ui-`);
await writeFile(
  `${folder}/Script.fountain`,
  "INT. CAFÉ - DAY\n\nA first draft.\n",
);
await writeFile(`${folder}/.gitignore`, ".test-config/\nprivate.md\n");
await writeFile(`${folder}/private.md`, "Private notes");
const backend = spawn(
  `${root}/target/debug/examples/workbench_host`,
  [folder],
  {
    stdio: ["pipe", "pipe", "inherit"],
    env: {
      ...process.env,
      GIT_CONFIG_GLOBAL: "/dev/null",
      GIT_CONFIG_NOSYSTEM: "1",
    },
  },
);
let serial = 0;
const pending = new Map();
createInterface({ input: backend.stdout }).on("line", (line) => {
  const r = JSON.parse(line),
    p = pending.get(r.request);
  pending.delete(r.request);
  r.error ? p.reject(Error(r.error)) : p.resolve(r.value);
});
const invoke = (name, args = {}) =>
  new Promise((resolve, reject) => {
    const request = ++serial;
    pending.set(request, { resolve, reject });
    backend.stdin.write(JSON.stringify({ request, name, args }) + "\n");
  });
const browser = await chromium.launch({ headless: true, channel: "chrome" });
const page = await browser.newPage({ viewport: { width: 1280, height: 840 } });
page.setDefaultTimeout(10000);
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
try {
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
    .getByRole("button", { name: "Open Project…", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Source Control", exact: true })
    .click();
  const panel = page.getByRole("region", {
    name: "Source Control",
    exact: true,
  });
  await panel
    .getByRole("button", { name: "Initialize Git", exact: true })
    .click();
  await panel
    .getByRole("button", { name: "Commit all changes", exact: true })
    .waitFor();
  await appendFile(`${folder}/.git/config`, "\n[user]\nname =\nemail =\n");
  await panel.getByRole("button", { name: "Refresh Git", exact: true }).click();
  await panel.getByLabel("Git name", { exact: true }).fill("Test Writer");
  await panel
    .getByLabel("Git email", { exact: true })
    .fill("writer@example.invalid");
  await panel
    .getByRole("button", { name: "Save Git identity", exact: true })
    .click();
  await panel
    .getByRole("button", { name: "Review changes", exact: true })
    .click();
  const diff = page.getByRole("region", { name: "Git diff", exact: true });
  await diff.getByText("Uncommitted changes", { exact: true }).waitFor();
  assert.match(
    await page.getByLabel("Git changes", { exact: true }).innerText(),
    /\+INT\. CAFÉ/,
  );
  const editorBox = await page.locator(".editor-pane").boundingBox(),
    diffBox = await diff.boundingBox();
  assert.ok(diffBox.x >= editorBox.x + editorBox.width - 2);
  assert.equal(await panel.getByText("private.md", { exact: true }).count(), 0);
  await panel.getByLabel("Commit message", { exact: true }).fill("First draft");
  await panel
    .getByRole("button", { name: "Commit all changes", exact: true })
    .click();
  await panel.getByText("No changes to commit.", { exact: true }).waitFor();
  assert.equal((await invoke("git_log", { root: folder })).length, 1);
  await mkdir(`${folder}/_river`, { recursive: true });
  await writeFile(`${folder}/_river/todos.md`, "- [ ] Rewrite ending\n");
  await writeFile(
    `${folder}/Script.fountain`,
    "INT. CAFÉ - NIGHT\n\nNew ending 🎬.\n",
  );
  await panel.getByRole("button", { name: "Refresh Git", exact: true }).click();
  await panel
    .getByRole("button", { name: "Review changes", exact: true })
    .click();
  assert.match(
    await page.getByLabel("Git changes", { exact: true }).innerText(),
    /\+INT\. CAFÉ - NIGHT/,
  );
  // Reject a commit when disk changed since the displayed review; keep the message.
  await writeFile(
    `${folder}/Script.fountain`,
    "INT. CAFÉ - NIGHT\n\nLater ending 🎬.\n",
  );
  await panel
    .getByLabel("Commit message", { exact: true })
    .fill("Rework ending");
  await panel
    .getByRole("button", { name: "Commit all changes", exact: true })
    .click();
  await panel
    .getByRole("alert")
    .filter({ hasText: "Project files changed" })
    .waitFor();
  assert.equal(
    await panel.getByLabel("Commit message", { exact: true }).inputValue(),
    "Rework ending",
  );
  assert.equal((await invoke("git_log", { root: folder })).length, 1);
  await panel
    .getByRole("button", { name: "Review changes", exact: true })
    .click();
  await panel
    .getByRole("button", { name: "Commit all changes", exact: true })
    .click();
  await panel.getByText("No changes to commit.", { exact: true }).waitFor();
  assert.equal((await invoke("git_log", { root: folder })).length, 2);
  await panel.getByRole("button").filter({ hasText: "First draft" }).click();
  await diff.getByText("First draft", { exact: true }).waitFor();
  assert.match(
    await page.getByLabel("Git changes", { exact: true }).innerText(),
    /\+INT\. CAFÉ - DAY/,
  );
  assert.match(
    await readFile(`${folder}/Script.fountain`, "utf8"),
    /Later ending/,
  );
  await page
    .getByRole("button", { name: "Script.fountain", exact: true })
    .count();
  await mkdir(`${root}/app/output`, { recursive: true });
  await page.screenshot({ path: `${root}/app/output/source-control.png` });
  assert.deepEqual(errors, []);
  console.log(
    "PASS: initialize, local identity, ignored files, ordinary commits, notes/tasks, side-pane diff, stale-review rejection and commit history.",
  );
} catch (e) {
  console.error(await page.locator("body").innerText());
  throw e;
} finally {
  await browser.close();
  backend.stdin.end();
  await rm(folder, { recursive: true, force: true });
}
