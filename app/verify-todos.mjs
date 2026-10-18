import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
export async function verifyTodos(page, folder, root) {
  const path = `${folder}/_river/todos.md`;
  const tabsBefore = await page.getByRole("tab").count();
  await page.getByRole("button", { name: "Todos", exact: true }).click();
  const panel = page.getByRole("region", { name: "Todos", exact: true });
  const entry = panel.getByRole("textbox", { name: "Add a task", exact: true });
  await entry.waitFor();
  for (const text of [
    "Fix café opening 🎬",
    "Rewrite ending",
    "Check dialogue",
  ]) {
    await entry.fill(text);
    await entry.press("Enter");
    await page.waitForFunction(
      () => !document.querySelector('[aria-label="Add a task"]').disabled,
    );
  }
  assert.equal(await page.getByRole("tab").count(), tabsBefore);
  assert.equal(
    await readFile(path, "utf8"),
    "- [ ] Fix café opening 🎬\n- [ ] Rewrite ending\n- [ ] Check dialogue\n",
  );
  await panel
    .getByRole("button", { name: "Fix café opening 🎬", exact: true })
    .click();
  const edit = panel.getByRole("textbox", { name: "Edit task", exact: true });
  await edit.fill("Polish café opening 🎬");
  await edit.press("Enter");
  await panel
    .getByRole("button", { name: "Polish café opening 🎬", exact: true })
    .waitFor();
  const grip = panel.getByRole("button", {
    name: "Reorder Check dialogue",
    exact: true,
  });
  await grip.focus();
  await grip.press("Alt+ArrowUp");
  await page.waitForFunction(
    () =>
      [...document.querySelectorAll(".task-list .task-title")]
        .map((n) => n.textContent)
        .join("|") === "Polish café opening 🎬|Check dialogue|Rewrite ending",
  );
  await panel
    .getByRole("button", { name: "Reorder Rewrite ending", exact: true })
    .dragTo(panel.locator(".task-row").first());
  await page.waitForFunction(
    () =>
      [...document.querySelectorAll(".task-list .task-title")]
        .map((n) => n.textContent)
        .join("|") === "Rewrite ending|Polish café opening 🎬|Check dialogue",
  );
  await panel
    .locator(".task-row")
    .filter({ hasText: "Rewrite ending" })
    .locator(".task-check-control")
    .click();
  await panel.locator("summary").filter({ hasText: "Completed (1)" }).waitFor();
  assert.equal(
    await panel
      .getByRole("checkbox", { name: "Reopen Rewrite ending", exact: true })
      .count(),
    0,
  );
  await panel.locator("summary").click();
  await panel
    .getByRole("checkbox", { name: "Reopen Rewrite ending", exact: true })
    .press("Space");
  await panel
    .getByRole("checkbox", { name: "Complete Rewrite ending", exact: true })
    .waitFor();
  await panel
    .getByRole("button", { name: "Delete Check dialogue", exact: true })
    .click();
  await page.waitForFunction(
    () =>
      !document
        .querySelector(".task-list")
        ?.textContent.includes("Check dialogue"),
  );
  await page.screenshot({ path: `${root}/app/output/todos.png` });
  // Leaving the panel commits a task still in quick entry; no editor tab is created.
  await entry.fill("Remember the river scene");
  await page.getByRole("button", { name: "Explorer", exact: true }).click();
  await page.getByRole("button", { name: "Todos", exact: true }).click();
  await panel
    .getByRole("button", { name: "Remember the river scene", exact: true })
    .waitFor();
  assert.equal(await page.getByRole("tab").count(), tabsBefore);
  // A concurrent external edit must win on disk; keep our entry so it can be retried.
  await entry.fill("My pending task");
  await writeFile(path, "# Outside River\n\n- [ ] External task\n");
  await entry.press("Enter");
  await panel
    .getByRole("alert")
    .filter({ hasText: "changed outside River" })
    .waitFor();
  assert.equal(
    await readFile(path, "utf8"),
    "# Outside River\n\n- [ ] External task\n",
  );
  assert.equal(await entry.inputValue(), "My pending task");
  await panel
    .getByRole("button", { name: "Reload tasks", exact: true })
    .click();
  await panel
    .getByRole("button", { name: "External task", exact: true })
    .waitFor();
  await entry.press("Enter");
  await panel
    .getByRole("button", { name: "My pending task", exact: true })
    .waitFor();
  assert.equal(
    await readFile(path, "utf8"),
    "# Outside River\n\n- [ ] External task\n- [ ] My pending task\n",
  );
  await page.getByRole("textbox", { name: "Story editor" }).click();
  await page.keyboard.press("ControlOrMeta+s");
  await page.waitForFunction(
    () =>
      !document
        .querySelector('[role="tab"][aria-selected="true"]')
        .textContent.includes("●"),
  );
  // Closing a project flushes a quick-entry draft and restores the Todos view.
  await entry.fill("Next session task");
  await entry.press("ControlOrMeta+Shift+w");
  await page.getByRole("main", { name: "Project welcome" }).waitFor();
  await page.locator(".recent-project").first().click();
  await panel
    .getByRole("button", { name: "Next session task", exact: true })
    .waitFor();
  assert.equal(await page.getByRole("tab").count(), tabsBefore);
  await page.getByRole("button", { name: "Explorer", exact: true }).click();
}
