import { expect, open, test } from "nsh/test";

test("shows the app name", async () => {
  const app = await open("./App.tsx");
  await app.waitFor("脉搏");
  expect(app.texts().some((text) => text.includes("脉搏"))).toBe(true);
  expect(app.texts().some((text) => text.includes("Pulse"))).toBe(true);
});
