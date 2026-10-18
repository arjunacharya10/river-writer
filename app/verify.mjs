import { chromium } from "playwright";
import { verifyWriting } from "./verify-writing.mjs";
import { verifyTodos } from "./verify-todos.mjs";
import { verifyScratchpad } from "./verify-scratchpad.mjs";
import { mkdtemp, readFile, writeFile, mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { root, startBackend, openPage } from "./test-harness.mjs";
import assert from "node:assert/strict";
const folder = await mkdtemp(`${tmpdir()}/river-workbench-`);
await mkdir(`${root}/app/output`, { recursive: true });
const source =
  "Title: Test River\n\nEXT. CAFÉ - DAY\n\nA **quiet** river.\n\nMAYA\n(softly)\nHello, world.\n\nINT. BOAT - NIGHT\n\nA light.\n";
await writeFile(`${folder}/Script.fountain`, source);
await writeFile(
  `${folder}/Chapter.md`,
  "# Chapter One\n\nCafé by the river.\n",
);
const backend = startBackend(folder);
const { invoke, calls } = backend;
let failures = [];
const browser = await chromium.launch({ headless: true, channel: "chrome" });
let page;
try {
  ({ page, errors: failures } = await openPage(browser, invoke));
  await page.getByRole("heading", { name: "River Writer" }).waitFor();
  await page.getByText("Your projects will appear here.").waitFor();
  assert.equal(
    await page.getByRole("navigation", { name: "Activity" }).isVisible(),
    false,
  );
  await page.screenshot({
    path: `${root}/app/output/project-welcome-empty.png`,
  });
  await page.evaluate(() => {
    window.__cancelNext = "create_project";
  });
  await page.getByRole("button", { name: "New Project…", exact: true }).click();
  await page.getByRole("main", { name: "Project welcome" }).waitFor();
  await page.getByRole("button", { name: "New Project…", exact: true }).click();
  await page.getByRole("tab", { name: /New Story.md/ }).waitFor();
  assert.equal((await page.locator(".cm-content").innerText()).trim(), "");
  assert.equal(await readFile(`${folder}/New Story/New Story.md`, "utf8"), "");
  await page.keyboard.press("ControlOrMeta+Shift+w");
  await page.getByRole("main", { name: "Project welcome" }).waitFor();
  await page
    .getByRole("button", { name: "Open Project…", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Script.fountain", exact: true })
    .click();
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
    await page
      .getByRole("region", { name: "Page preview", exact: true })
      .getByRole("button")
      .count(),
    0,
  );
  await page.mouse.move(0, 0);
  await page
    .getByRole("button", { name: "Toggle page preview", exact: true })
    .hover();
  const tip = page
    .getByRole("tooltip")
    .filter({ hasText: "Toggle page preview" });
  await tip.waitFor();
  assert.equal(
    await tip.evaluate((node) => getComputedStyle(node).pointerEvents),
    "none",
  );
  await page.locator(".preview-scroll").evaluate((node) => {
    node.scrollTop = node.scrollHeight;
  });
  await page.getByAltText("Page 2", { exact: true }).waitFor();
  await page.locator(".preview-scroll").evaluate((node) => {
    node.scrollTop = 0;
  });
  // Repeated hide/show must preserve both source visibility and real pane geometry.
  // Reading .cm-content alone misses a zero-width editor containing intact text.
  const toggle = page.getByRole("button", {
    name: "Toggle page preview",
    exact: true,
  });
  async function assertPanes(open) {
    await page.waitForFunction((open) => {
      const editor = document
        .querySelector(".editor-pane")
        .getBoundingClientRect();
      const preview = document
        .querySelector(".preview-pane")
        .getBoundingClientRect();
      const split = document
        .querySelector(".editor-split")
        .getBoundingClientRect();
      const text = document.querySelector(".cm-line").getBoundingClientRect();
      return (
        editor.width >= 219 &&
        text.width > 0 &&
        text.right > editor.left &&
        text.left < editor.right &&
        (open
          ? preview.width >= 219 &&
            editor.right <= preview.left + 1 &&
            preview.right <= split.right + 1
          : preview.width < 1 && editor.width >= split.width - 1)
      );
    }, open);
    assert.match(await page.locator(".cm-content").innerText(), /CAFÉ/);
  }
  for (const width of [800, 1512, 1280]) {
    await page.setViewportSize({ width, height: 840 });
    for (let n = 0; n < 3; n++) {
      await toggle.click();
      await assertPanes(false);
      await toggle.click();
      await assertPanes(true);
    }
  }
  const divider = page.getByRole("separator", { name: "Resize side pane" });
  await divider.focus();
  for (let n = 0; n < 15; n++) await page.keyboard.press("ArrowLeft");
  await assertPanes(true);
  assert.ok((await page.locator(".editor-pane").boundingBox()).width < 225);
  for (let n = 0; n < 15; n++) await page.keyboard.press("ArrowRight");
  await assertPanes(true);
  assert.ok((await page.locator(".preview-pane").boundingBox()).width < 225);
  await toggle.click();
  await toggle.click();
  await assertPanes(true);
  // Sidebar hide/show uses the same stable registration and must not shrink the editor.
  for (let n = 0; n < 3; n++) {
    await page.keyboard.press("ControlOrMeta+b");
    await assertPanes(true);
    await page.keyboard.press("ControlOrMeta+b");
    await assertPanes(true);
  }
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

  // Explorer New File: inline name, .md added, opens empty; never replaces a file.
  const newFile = () =>
    page.getByRole("button", { name: "New File…", exact: true }).click();
  const nameField = page.getByLabel("New file name", { exact: true });
  await newFile();
  await nameField.fill("Scene Notes");
  await nameField.press("Enter");
  await page.getByRole("tab", { name: /Scene Notes.md/ }).waitFor();
  await page
    .getByRole("button", { name: "Scene Notes.md", exact: true })
    .waitFor();
  assert.equal(await readFile(`${folder}/Scene Notes.md`, "utf8"), "");
  assert.equal(await nameField.count(), 0);
  await newFile();
  await nameField.fill("Scene Notes.md");
  await nameField.press("Enter");
  await page
    .getByRole("alert")
    .filter({ hasText: "A file named Scene Notes.md already exists." })
    .waitFor();
  await page.screenshot({ path: `${root}/app/output/new-file.png` });
  await nameField.press("Escape");
  assert.equal(await nameField.count(), 0);
  await newFile();
  await nameField.press("Escape");
  assert.equal(await nameField.count(), 0);
  await page
    .getByRole("button", { name: "Close Scene Notes.md", exact: true })
    .click();
  await rm(`${folder}/Scene Notes.md`);

  await page.getByRole("button", { name: "Chapter.md", exact: true }).click();
  await page.getByRole("tab", { name: /Chapter.md/ }).waitFor();
  await page.getByRole("tab", { name: /Script.fountain/ }).click();
  await page.waitForFunction(() =>
    document
      .querySelector('[role="tab"][aria-selected="true"]')
      .textContent.includes("Script.fountain"),
  );
  assert.match(await page.locator(".cm-content").innerText(), /CAFÉ/);
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
  assert.equal(
    await page
      .getByRole("button", { name: "Toggle Light / Dark", exact: true })
      .count(),
    0,
  );
  assert.equal(
    await page
      .getByRole("button", { name: "Command Palette", exact: true })
      .count(),
    0,
  );
  const themeBefore = await page.locator("html").getAttribute("data-theme");
  await page.keyboard.press("ControlOrMeta+Shift+p");
  await page
    .getByRole("textbox", { name: "Search commands" })
    .fill("Toggle Light");
  await page
    .getByRole("button", { name: "View: Toggle Light / Dark", exact: true })
    .click();
  assert.notEqual(
    await page.locator("html").getAttribute("data-theme"),
    themeBefore,
  );
  await page.keyboard.press("ControlOrMeta+Shift+p");
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
  // A dirty project cannot be replaced when the discard dialog is cancelled.
  await page.keyboard.press("ControlOrMeta+Shift+w");
  await page.getByRole("tab", { name: /Script.fountain/ }).waitFor();
  await page.keyboard.press("ControlOrMeta+s");
  await page.waitForFunction(() =>
    document.querySelector(".status-bar").textContent.includes("Saved"),
  );
  await page.waitForFunction(
    () =>
      document
        .querySelector('[role="tab"][aria-selected="true"]')
        .textContent.includes("Script.fountain") &&
      !document
        .querySelector('[role="tab"][aria-selected="true"]')
        .textContent.includes("●"),
  );
  const theme = await page.locator("html").getAttribute("data-theme");
  const editorWidthBefore = (await page.locator(".editor-pane").boundingBox())
    .width;
  await page.keyboard.press("ControlOrMeta+Shift+w");
  await page.getByRole("main", { name: "Project welcome" }).waitFor();
  await page.locator(".recent-project").first().waitFor();
  await page.screenshot({
    path: `${root}/app/output/project-welcome-recents.png`,
  });
  await page.locator(".recent-project").first().click();
  await page.getByRole("tab", { name: /Script.fountain/ }).waitFor();
  assert.equal(await page.getByRole("tab").count(), 2);
  assert.equal(
    await page
      .getByRole("tab", { name: /Script.fountain/ })
      .getAttribute("aria-selected"),
    "true",
  );
  assert.equal(await page.locator("html").getAttribute("data-theme"), theme);
  await page
    .getByRole("button", { name: "Page template" })
    .filter({ hasText: "Stage Play" })
    .waitFor();
  await page.getByRole("region", { name: "Page preview" }).waitFor();
  assert.equal(await page.locator(".cm-content").innerText(), before);
  assert.ok(
    Math.abs(
      (await page.locator(".editor-pane").boundingBox()).width -
        editorWidthBefore,
    ) < 4,
  );
  await verifyTodos(page, folder, root);
  await verifyScratchpad(page, folder, root);
  await verifyWriting(page, root);
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
        "project welcome, cancelled creation, project open, Explorer New File, recents, session restore, open, edit, save, Rust preview, repeated pane toggles, minimum widths, window resizing, tooltips, outline, tabs, undo, templates, themes, palette, find, keyboard resize, dirty close, PDF, external changes, fonts, screenplay completions/capitals, manuscript typing, Todos CRUD/reorder/completion/conflict safety, Scratchpad autosave/recovery/conflict copy/session restore",
      result: "passed",
    }),
  );
} catch (error) {
  if (page) {
    await page.screenshot({ path: `${root}/app/output/verify-failure.png` });
    await writeFile(
      `${root}/app/output/verify-failure.txt`,
      JSON.stringify(
        { body: await page.locator("body").innerText(), failures, calls },
        null,
        2,
      ),
    );
  }
  throw error;
} finally {
  await browser.close();
  backend.stop();
  await rm(folder, { recursive: true, force: true });
}
