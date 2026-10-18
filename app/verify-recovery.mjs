import { existsSync } from "node:fs";
import { chromium } from "playwright";
import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { startBackend, openPage, until } from "./test-harness.mjs";
import assert from "node:assert/strict";
const folder = await mkdtemp(`${tmpdir()}/river-recovery-ui-`);
const source = `${folder}/Script.fountain`;
const journals = `${folder}/.test-config/recovery`;
await writeFile(source, "Original story");
let backend;
const start = () => (backend = startBackend(folder));
const invoke = (name, args) => backend.invoke(name, args);
const crash = () => backend.crash();
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
  const { page, errors } = await openPage(browser, invoke);
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
  // A named draft saves after idle typing, including when its tab is inactive.
  await page.locator(".cm-content").click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.insertText(" automatically");
  await page.getByRole("tab", { name: /Untitled.md/ }).click();
  await until(
    async () =>
      (await readFile(`${folder}/Saved.md`, "utf8")) ===
      "My untitled draft continued automatically",
  );
  await page.getByRole("tab", { name: /Saved.md/ }).click();
  // Autosave writes the file only: no private history store or Timeline (#66).
  assert.equal(
    await page.getByRole("button", { name: "Toggle local history" }).count(),
    0,
  );
  assert.equal(existsSync(`${folder}/.git/river-autosave`), false);
  assert.equal(existsSync(`${folder}/.test-config/history`), false);
  // An external edit blocks autosave and leaves our own writing in recovery.
  await writeFile(`${folder}/Saved.md`, "External replacement");
  await page.locator(".cm-content").click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.insertText(" private changes");
  await page
    .getByRole("alert")
    .filter({ hasText: "Could not automatically save" })
    .waitFor();
  assert.equal(
    await readFile(`${folder}/Saved.md`, "utf8"),
    "External replacement",
  );
  await until(async () => {
    const files = await readdir(journals);
    const drafts = await Promise.all(
      files
        .filter((f) => f.endsWith(".json"))
        .map(async (f) =>
          JSON.parse(await readFile(`${journals}/${f}`, "utf8")),
        ),
    );
    return drafts.some((d) => d.text.endsWith(" private changes"));
  });
  assert.deepEqual(errors, []);
  console.log(
    "PASS: abrupt service termination, named and untitled recovery, external edit preserved, continued writing journaled, saving clears recovery, inactive-tab autosave without private history, external-change protection.",
  );
} finally {
  await browser.close();
  if (backend) await crash();
  await rm(folder, { recursive: true, force: true });
}
