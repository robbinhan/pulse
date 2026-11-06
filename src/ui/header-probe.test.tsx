import { expect, open, test } from "nsh/test";

test("the account menu opens from a plain trigger", async () => {
  const app = await open("./header-probe.tsx", { width: 800, height: 600, platform: "macos" });
  await app.waitFor("账号菜单");
  expect(app.texts()).not.toContain("编辑账号");
  await app.press("账号菜单");
  await app.waitFor("编辑账号");
  expect(app.texts()).toContain("编辑账号");
});

test("the unread badge hides before the save runs", async () => {
  const app = await open("./header-probe.tsx", { width: 800, height: 600, platform: "macos" });
  await app.waitFor("7");
  await app.press("把这个格子标为已读");
  await app.waitUntil(() => !app.texts().includes("7"));
  expect(app.texts()).not.toContain("7");
});
