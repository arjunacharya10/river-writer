import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { mkdtemp, readFile, writeFile, mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
const root = fileURLToPath(new URL("../", import.meta.url));
const folder = await mkdtemp(`${tmpdir()}/river-workbench-`);
await mkdir(`${root}/app/output`, { recursive: true });
const source =
  "Title: Test River\n\nEXT. CAFÉ - DAY\n\nA **quiet** river.\n\nMAYA\n(softly)\nHello, world.\n\nINT. BOAT - NIGHT\n\nA light.\n";
await writeFile(`${folder}/Script.fountain`, source);
await writeFile(
  `${folder}/Chapter.md`,
  "# Chapter One\n\nCafé by the river.\n",
);
const backend = spawn(
  `${root}/target/debug/examples/workbench_host`,
  [folder],
  { stdio: ["pipe", "pipe", "inherit"] },
);
const requests = new Map();
let request = 0;
const failures = [];
createInterface({ input: backend.stdout }).on("line", (line) => {
  const reply = JSON.parse(line),
    pending = requests.get(reply.request);
  requests.delete(reply.request);
  reply.error
    ? pending.reject(Error(reply.error))
    : pending.resolve(reply.value);
});
backend.on("exit", (code) => {
  for (const p of requests.values()) p.reject(Error(`Backend exited ${code}`));
});
const invoke = (name, args = {}) =>
  new Promise((resolve, reject) => {
    const id = ++request;
    requests.set(id, { resolve, reject });
    backend.stdin.write(JSON.stringify({ request: id, name, args }) + "\n");
  });
const browser = await chromium.launch({ headless: true, channel: "chrome" });
try {
  const page = await browser.newPage({
    viewport: { width: 1280, height: 840 },
  });
  page.on("pageerror", (e) => failures.push(String(e)));
  page.on("response", (r) => {
    if (r.status() >= 400) failures.push(`${r.status()} ${r.url()}`);
  });
  await page.exposeBinding("riverInvoke", (_, name, args) =>
    invoke(name, args),
  );
  await page.addInitScript(() => {
    window.__RIVER_TEST__ = {
      invoke: async (name, args) => {
        const result = await window.riverInvoke(name, args);
        return name === "render_page" ? new Uint8Array(result).buffer : result;
      },
    };
  });
  await page.goto("http://127.0.0.1:1421/");
  await page.getByRole("heading", { name: "River Writer" }).waitFor();
  await page.getByRole("button", { name: "Open File… ⌘O" }).click();
  await page.getByRole("tab", { name: /Script.fountain/ }).waitFor();
  await page.locator(".story-cue").waitFor();
  assert.match(await page.locator(".cm-content").innerText(), /CAFÉ/);
  await page
    .getByRole("button", { name: "Toggle page preview", exact: true })
    .click();
  await page.getByAltText("Page 1", { exact: true }).waitFor();
  assert.equal(
    await page
      .getByAltText("Page 1", { exact: true })
      .evaluate((img) => img.naturalWidth),
    1224,
  );
  assert.equal(
    await page.getByLabel("Current page", { exact: true }).innerText(),
    "1",
  );
  await page.getByRole("button", { name: "Next page", exact: true }).click();
  await page.waitForFunction(
    () =>
      document.querySelector('[aria-label="Current page"]').textContent === "2",
  );
  await page
    .getByRole("button", { name: "Previous page", exact: true })
    .click();
  await page.waitForFunction(
    () =>
      document.querySelector('[aria-label="Current page"]').textContent === "1",
  );
  const editor = page.getByRole("textbox", { name: "Story editor" });
  await editor.click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.type("\nNew words.");
  await page.waitForFunction(
    () =>
      document.querySelector(".story-action") &&
      document.querySelector(".cm-content").innerText.includes("New words."),
  );
  await page.keyboard.press("ControlOrMeta+s");
  await page.waitForFunction(() =>
    document.querySelector(".status-bar").textContent.includes("Saved"),
  );
  assert.match(
    await readFile(`${folder}/Script.fountain`, "utf8"),
    /New words/,
  );
  await page.getByRole("button", { name: "Outline", exact: true }).click();
  await page.getByRole("button", { name: "INT. BOAT - NIGHT" }).waitFor();
  await page.getByRole("button", { name: "Explorer", exact: true }).click();
  await page
    .getByRole("button", { name: "Open Folder", exact: true })
    .first()
    .click();
  await page.getByRole("button", { name: "Chapter.md", exact: true }).click();
  await page.getByRole("tab", { name: /Chapter.md/ }).waitFor();
  await page.getByRole("tab", { name: /Script.fountain/ }).click();
  await editor.click();
  await page.keyboard.press("ControlOrMeta+z");
  assert.doesNotMatch(
    await page.locator(".cm-content").innerText(),
    /New words/,
  );
  // Template changes affect geometry and saved UI state, never the story text.
  const before = await page.locator(".cm-content").innerText();
  await page.getByRole("button", { name: "Page template" }).click();
  await page.getByRole("option", { name: "Stage Play" }).click();
  await page.waitForFunction(() =>
    document
      .querySelector(".template-picker")
      .textContent.includes("Stage Play"),
  );
  assert.equal(await page.locator(".cm-content").innerText(), before);
  await page
    .getByRole("button", { name: "Toggle Light / Dark", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Command Palette", exact: true })
    .click();
  await page.getByRole("textbox", { name: "Search commands" }).fill("Find");
  await page.getByRole("button", { name: /Edit: Find/ }).click();
  await page.locator(".cm-search").waitFor();
  await page.keyboard.press("Escape");
  // Keyboard resizing must change the sidebar width.
  const handle = page.getByRole("separator", { name: "Resize sidebar" });
  const width = await page.locator(".sidebar").evaluate((e) => e.clientWidth);
  await handle.focus();
  await page.keyboard.press("ArrowRight");
  const resized = await page.locator(".sidebar").evaluate((e) => e.clientWidth);
  assert.notEqual(width, resized);
  // A dirty close can be cancelled without losing the tab.
  await page
    .getByRole("button", { name: "Close Script.fountain", exact: true })
    .click();
  assert.equal(
    await page.getByRole("tab", { name: /Script.fountain/ }).count(),
    1,
  );
  await page
    .getByRole("button", { name: "Export PDF", exact: true })
    .first()
    .click();
  await page.waitForFunction(() =>
    document.querySelector(".status-bar").textContent.includes("Exported"),
  );
  assert.match(
    (await readFile(`${folder}/Export.pdf`)).subarray(0, 5).toString(),
    /%PDF-/,
  );
  // External changes remain intact when Save is attempted.
  await writeFile(`${folder}/Script.fountain`, "!Changed in another editor.\n");
  await page
    .getByText("This file changed outside River.", { exact: false })
    .waitFor();
  await page.keyboard.press("ControlOrMeta+s");
  await page
    .getByRole("alert")
    .filter({ hasText: "File changed outside River" })
    .waitFor();
  assert.equal(
    await readFile(`${folder}/Script.fountain`, "utf8"),
    "!Changed in another editor.\n",
  );
  for (const face of ["normal 400", "normal 700", "italic 400", "italic 700"]) {
    await page.evaluate(async (face) => {
      await document.fonts.load(`${face} 16px "Courier Prime"`);
    }, face);
  }
  assert.equal(
    await page.evaluate(() =>
      document.fonts.check('700 italic 16px "Courier Prime"'),
    ),
    true,
  );
  await page.screenshot({ path: `${root}/app/output/workbench-dark.png` });
  assert.deepEqual(failures, []);
  console.log(
    JSON.stringify({
      checks:
        "open, edit, save, Rust preview, outline, tabs, undo, templates, themes, palette, find, keyboard resize, dirty close, PDF, external changes, fonts",
      result: "passed",
    }),
  );
} finally {
  await browser.close();
  backend.stdin.end();
  await rm(folder, { recursive: true, force: true });
}
