import assert from "node:assert/strict";
import { readFile, writeFile, readdir } from "node:fs/promises";
import { until } from "./test-harness.mjs";
export async function verifyScratchpad(page, folder, root) {
  assert.equal(
    await page.getByRole("button", { name: "Bin", exact: true }).count(),
    0,
  );
  const before = await page.getByRole("tab").count(),
    path = `${folder}/_river/scratchpad.md`;
  await page.getByRole("button", { name: "Scratchpad", exact: true }).click();
  const panel = page.getByRole("region", { name: "Scratchpad", exact: true });
  const notes = panel.getByRole("textbox", {
    name: "Scratchpad notes",
    exact: true,
  });
  await notes.fill("Loose café notes 🎬\nA possible ending.");
  await until(
    async () =>
      (await readFile(path, "utf8")) ===
      "Loose café notes 🎬\nA possible ending.",
  );
  await page.screenshot({ path: `${root}/app/output/scratchpad.png` });
  await page.getByRole("button", { name: "Todos", exact: true }).click();
  await page.getByRole("button", { name: "Scratchpad", exact: true }).click();
  assert.equal(
    await notes.inputValue(),
    "Loose café notes 🎬\nA possible ending.",
  );
  assert.equal(await page.getByRole("tab").count(), before);
  await notes.fill("My competing notes 🎬");
  await writeFile(path, "External scratchpad.\n");
  await panel
    .getByRole("alert")
    .filter({ hasText: "changed outside River" })
    .waitFor();
  assert.equal(await readFile(path, "utf8"), "External scratchpad.\n");
  const journal = `${folder}/.test-config/recovery`;
  await until(async () => {
    const files = await readdir(journal);
    const drafts = await Promise.all(
      files
        .filter((name) => name.endsWith(".json"))
        .map(async (name) =>
          JSON.parse(await readFile(`${journal}/${name}`, "utf8")),
        ),
    );
    return drafts.some(
      (draft) =>
        draft.text === "My competing notes 🎬" &&
        draft.path.endsWith("scratchpad.md"),
    );
  });
  await page.evaluate(() => {
    window.__cancelNext = "copy_scratchpad";
  });
  await panel
    .getByRole("button", { name: "Save a copy and reload", exact: true })
    .click();
  await until(async () => !(await notes.isDisabled()));
  assert.equal(await notes.inputValue(), "My competing notes 🎬");
  await panel
    .getByRole("button", { name: "Save a copy and reload", exact: true })
    .click();
  await until(
    async () => (await notes.inputValue()) === "External scratchpad.\n",
  );
  assert.equal(
    await readFile(`${folder}/Scratchpad copy.md`, "utf8"),
    "My competing notes 🎬",
  );
  assert.equal(await readFile(path, "utf8"), "External scratchpad.\n");
  await notes.fill("Saved when leaving the sidebar.");
  await page.getByRole("button", { name: "Explorer", exact: true }).click();
  await until(
    async () =>
      (await readFile(path, "utf8")) === "Saved when leaving the sidebar.",
  );
  await page.getByRole("button", { name: "Scratchpad", exact: true }).click();
  await notes.fill("Notes for next session.");
  await notes.press("ControlOrMeta+Shift+w");
  await page.getByRole("main", { name: "Project welcome" }).waitFor();
  await page.locator(".recent-project").first().click();
  await until(
    async () => (await notes.inputValue()) === "Notes for next session.",
  );
  assert.equal(await page.getByRole("tab").count(), before);
  await page.getByRole("button", { name: "Explorer", exact: true }).click();
}
