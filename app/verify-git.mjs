import { chromium } from "playwright";
import {
  mkdtemp,
  writeFile,
  appendFile,
  readFile,
  rm,
  mkdir,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { execFileSync } from "node:child_process";
import { root, startBackend, openPage, until } from "./test-harness.mjs";
import assert from "node:assert/strict";
const folder = await mkdtemp(`${tmpdir()}/river-git-ui-`);
// Existing writing in a folder that has never used Git.
await writeFile(
  `${folder}/Script.fountain`,
  "INT. CAFÉ - DAWN\n\nAn outline.\n",
);
await writeFile(`${folder}/.gitignore`, ".test-config/\nprivate.md\n");
await writeFile(`${folder}/private.md`, "Private notes");
const backend = startBackend(folder, {
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
});
const { invoke } = backend;
const browser = await chromium.launch({ headless: true, channel: "chrome" });
const { page, errors } = await openPage(browser, invoke);
try {
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
  // Git starts on its own: one snapshot, nothing pending, .gitignore untouched.
  await panel.getByText("No changes to commit.", { exact: true }).waitFor();
  assert.equal(
    await panel.getByRole("button", { name: /Initialize/ }).count(),
    0,
  );
  await panel
    .getByRole("button")
    .filter({ hasText: "Start project history" })
    .waitFor();
  assert.equal(
    await readFile(`${folder}/.gitignore`, "utf8"),
    ".test-config/\nprivate.md\n",
  );
  assert.match(
    await readFile(`${folder}/.git/info/exclude`, "utf8"),
    /\/_river\/river\.json/,
  );
  assert.equal(await panel.getByLabel("New branch name").count(), 0);
  // The writer's own edit is the only change listed.
  await writeFile(
    `${folder}/Script.fountain`,
    "INT. CAFÉ - DAY\n\nA first draft.\n",
  );
  await panel.getByRole("button", { name: "Refresh Git", exact: true }).click();
  await panel.getByText("CHANGES · 1", { exact: true }).waitFor();
  await appendFile(`${folder}/.git/config`, "\n[user]\nname =\nemail =\n");
  await panel.getByRole("button", { name: "Refresh Git", exact: true }).click();
  // No Git identity: River uses the Mac account's name and never asks.
  await panel.getByText("CHANGES · 1", { exact: true }).waitFor();
  assert.equal(await panel.getByLabel("Git name").count(), 0);
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
  assert.equal((await invoke("git_log", { root: folder })).length, 2);
  assert.equal(
    (await invoke("git_log", { root: folder }))[0].author,
    execFileSync("/usr/bin/id", ["-F"], { encoding: "utf8" }).trim(),
  );
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
  // Review is fetched on demand; wait for the fresh diff to replace the old one.
  await page
    .getByLabel("Git changes", { exact: true })
    .getByText("+INT. CAFÉ - NIGHT", { exact: true })
    .waitFor();
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
  assert.equal((await invoke("git_log", { root: folder })).length, 2);
  await panel
    .getByRole("button", { name: "Review changes", exact: true })
    .click();
  await panel
    .getByRole("button", { name: "Commit all changes", exact: true })
    .click();
  await panel.getByText("No changes to commit.", { exact: true }).waitFor();
  assert.equal((await invoke("git_log", { root: folder })).length, 3);
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
  // Restore one file from a commit, with an open tab, then undo it.
  const story = `${folder}/Script.fountain`;
  const editorText = () => page.locator(".cm-content").innerText();
  await diff
    .getByRole("button", { name: "Restore Script.fountain", exact: true })
    .click();
  const confirm = diff.getByRole("alertdialog", { name: "Confirm restore" });
  await confirm.getByRole("button", { name: "Restore", exact: true }).click();
  await page
    .getByRole("status")
    .filter({ hasText: "Restored Script.fountain" })
    .waitFor();
  assert.match(await readFile(story, "utf8"), /A first draft/);
  await until(async () => /A first draft/.test(await editorText()));
  assert.equal((await invoke("git_log", { root: folder })).length, 3);
  await page.screenshot({ path: `${root}/app/output/git-restore.png` });
  await page.getByRole("button", { name: "Undo restore", exact: true }).click();
  await until(async () => /Later ending/.test(await readFile(story, "utf8")));
  await until(async () => /Later ending/.test(await editorText()));
  // Whole project: removes files added since, then undo brings them back.
  await diff
    .getByRole("button", { name: "Restore project…", exact: true })
    .click();
  await confirm.getByRole("button", { name: "Restore", exact: true }).click();
  await page
    .getByRole("status")
    .filter({ hasText: "Restored 2 files" })
    .waitFor();
  assert.match(await readFile(story, "utf8"), /A first draft/);
  await assert.rejects(readFile(`${folder}/_river/todos.md`, "utf8"));
  assert.equal(await readFile(`${folder}/private.md`, "utf8"), "Private notes");
  await page.getByRole("button", { name: "Undo restore", exact: true }).click();
  await until(async () => {
    try {
      return (
        (await readFile(`${folder}/_river/todos.md`, "utf8")) ===
        "- [ ] Rewrite ending\n"
      );
    } catch {
      return false;
    }
  });
  assert.match(await readFile(story, "utf8"), /Later ending/);
  // Undo never overwrites writing done after a restore.
  await confirm.count();
  await diff
    .getByRole("button", { name: "Restore Script.fountain", exact: true })
    .click();
  await confirm.getByRole("button", { name: "Restore", exact: true }).click();
  await page
    .getByRole("status")
    .filter({ hasText: "Restored Script.fountain" })
    .waitFor();
  await writeFile(story, "Newer writing after the restore.\n");
  await page.getByRole("button", { name: "Undo restore", exact: true }).click();
  await page
    .getByRole("alert")
    .filter({ hasText: "changed after the restore" })
    .waitFor();
  assert.equal(
    await readFile(story, "utf8"),
    "Newer writing after the restore.\n",
  );
  assert.equal((await invoke("git_log", { root: folder })).length, 3);
  // Branches: create with uncommitted work, commit on it, switch back and forth.
  const statusBranch = page.locator(".status-branch");
  await until(async () => (await statusBranch.innerText()).includes("main"));
  await statusBranch.click();
  await page
    .getByRole("menuitem", { name: "New branch…", exact: true })
    .click();
  await page.getByLabel("New branch name", { exact: true }).fill("draft-2");
  await page
    .getByRole("button", { name: "Create and switch", exact: true })
    .click();
  await until(async () => (await statusBranch.innerText()).includes("draft-2"));
  assert.equal(
    await readFile(story, "utf8"),
    "Newer writing after the restore.\n",
  );
  await statusBranch.click();
  await page
    .getByRole("menuitem", { name: "✓ draft-2", exact: true })
    .waitFor();
  await page.keyboard.press("Escape");
  await panel
    .getByLabel("Commit message", { exact: true })
    .fill("Draft two ending");
  await panel
    .getByRole("button", { name: "Commit all changes", exact: true })
    .click();
  await panel.getByText("No changes to commit.", { exact: true }).waitFor();
  await statusBranch.click();
  await page.getByRole("menuitem", { name: "main", exact: true }).click();
  await until(async () => (await statusBranch.innerText()).includes("main"));
  await until(async () => /Later ending/.test(await readFile(story, "utf8")));
  await until(async () => /Later ending/.test(await editorText()));
  assert.equal((await invoke("git_log", { root: folder })).length, 3);
  await page.screenshot({ path: `${root}/app/output/git-branches.png` });
  // Uncommitted writing blocks switching to a diverged branch, and stays put.
  await writeFile(story, "Unfinished thought on main.\n");
  await panel.getByRole("button", { name: "Refresh Git", exact: true }).click();
  await statusBranch.click();
  await page.getByRole("menuitem", { name: "draft-2", exact: true }).click();
  await page
    .getByRole("alert")
    .filter({ hasText: "Commit your changes before switching" })
    .waitFor();
  assert.equal(await readFile(story, "utf8"), "Unfinished thought on main.\n");
  assert.match(await statusBranch.innerText(), /main/);
  await mkdir(`${root}/app/output`, { recursive: true });
  await page.screenshot({ path: `${root}/app/output/source-control.png` });
  assert.deepEqual(errors, []);
  console.log(
    "PASS: Git on by default with a starting snapshot and untouched .gitignore, local identity, ignored files, ordinary commits, notes/tasks, side-pane diff, stale-review rejection, Mac-account identity without a form, commit history, file/project restore, undo and edit-after-restore protection, branch create/switch/status bar and dirty-switch refusal.",
  );
} catch (e) {
  console.error(await page.locator("body").innerText());
  throw e;
} finally {
  await browser.close();
  backend.stop();
  await rm(folder, { recursive: true, force: true });
}
