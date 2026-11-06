import { expect, open, test } from "nsh/test";

test("a weibo profile url becomes a saved card", async () => {
  const app = await open("./App.tsx", { width: 1100, height: 800, platform: "macos" });
  await app.waitFor("添加账号");
  await app.press("添加账号 >> nth=0");
  await app.waitFor("添加微博账号");
  await app.press("添加微博账号");
  await app.waitFor("完成");
  await app.type("text:昵称、数字 uid，或主页链接", "https://weibo.com/u/1319518957");
  await app.press("完成");
  await app.waitUntil(() => {
    const raw = app.storage()["theysay.profile.v1"];
    if (!raw) return false;
    try {
      const profile = JSON.parse(raw) as { weibo?: { cards?: { handle?: string }[] } };
      return profile.weibo?.cards?.some((card) => card.handle === "1319518957") === true;
    } catch {
      return false;
    }
  });
});
