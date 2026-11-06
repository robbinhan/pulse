import { expect, open, test } from "nsh/test";

test("a card list that is not pinned stays put when a post is added", async () => {
  const app = await open("./feed-scroll.tsx", { height: 844, width: 390, platform: "macos" });
  await app.press("id:open");
  await app.waitUntil(() => {
    const raw = app.texts().find((text) => /^\d+$/.test(text));
    return raw != null && Number(raw) > 400;
  });
  const beforeOpen = Number(app.texts().find((text) => /^\d+$/.test(text)));
  await app.scroll("id:feed", -500);
  await app.waitUntil(() => {
    const raw = app.texts().find((text) => /^\d+$/.test(text));
    return raw != null && Number(raw) < beforeOpen - 100;
  });
  const parked = Number(app.texts().find((text) => /^\d+$/.test(text)));
  await app.press("id:add");
  await app.wait(50);
  const after = Number(app.texts().find((text) => /^\d+$/.test(text)));
  expect(Math.abs(after - parked)).toBeLessThan(30);
  expect(after).toBeLessThan(beforeOpen - 100);
});
