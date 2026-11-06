import { expect, open, test } from "nsh/test";

test("settings lists each platform login", async () => {
  const app = await open("./App.tsx", { width: 1100, height: 800, platform: "macos" });
  await app.waitFor("打开设置");
  await app.press("打开设置");
  await app.waitFor("账号登录");
  await app.waitFor("云端同步");
  const texts = app.texts();
  expect(texts.some((text) => text.includes("账号登录"))).toBe(true);
  expect(texts.some((text) => text.includes("云端同步"))).toBe(true);
  expect(texts.some((text) => text === "X")).toBe(true);
  expect(texts.some((text) => text === "Threads")).toBe(true);
  expect(texts.some((text) => text === "Instagram")).toBe(true);
  expect(texts.some((text) => text === "微博")).toBe(true);
  expect(texts.some((text) => text === "RSS")).toBe(true);
  expect(texts.some((text) => text.includes("不用登录"))).toBe(true);
  expect(texts.filter((text) => text === "登录").length).toBe(4);
});
