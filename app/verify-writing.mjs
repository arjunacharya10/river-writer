import assert from "node:assert/strict";
export async function verifyWriting(page, root) {
  const editor = page.getByRole("textbox", { name: "Story editor" });
  await editor.click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.type("@ma");
  const maya = page.getByRole("option", { name: /MAYA/ });
  await maya.waitFor();
  await page.screenshot({ path: `${root}/app/output/writing-completion.png` });
  await maya.click();
  await page.waitForFunction(() =>
    document.querySelector(".cm-content").textContent.endsWith("@MAYA"),
  );
  // An exact completed name must allow the next Enter to start dialogue.
  await page.keyboard.press("Enter");
  await page.keyboard.type("Hello from the river.");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.type("int. ca");
  await page.getByRole("option", { name: /CAFÉ/ }).waitFor();
  await page.keyboard.press("Tab");
  await page.waitForFunction(() =>
    document.querySelector(".cm-content").textContent.endsWith("INT. CAFÉ"),
  );
  await page.keyboard.type(" - ni");
  await page.getByRole("option", { name: /NIGHT/ }).waitFor();
  await page.keyboard.press("Enter");
  await page.waitForFunction(() =>
    document
      .querySelector(".cm-content")
      .textContent.endsWith("INT. CAFÉ - NIGHT"),
  );
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.type("@jo");
  // This is a new name; Escape must leave literal writing available.
  await page.keyboard.press("Escape");
  await page.keyboard.type("n");
  assert.match(await editor.innerText(), /@JON$/);
  await page.getByRole("tab", { name: /Chapter.md/ }).click();
  await editor.click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.type("int. room");
  assert.match(await editor.innerText(), /int\. room$/);
  assert.equal(await page.getByRole("listbox").count(), 0);
  await page.getByRole("tab", { name: /Script.fountain/ }).click();
}
