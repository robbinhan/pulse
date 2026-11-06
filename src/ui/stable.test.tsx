import { expect, test } from "nsh/test";
import { postFp, reuseByKey } from "./stable";

test("a refresh that repeats the same posts keeps the row objects", () => {
  const kept = new Map<string, { key: string; text: string }>();
  const first = [
    { key: "a", text: "one" },
    { key: "b", text: "two" },
  ];
  const opened = reuseByKey(
    kept,
    [],
    first,
    (item) => item.key,
    (item) => item.text,
  );
  expect(opened.changed).toBe(true);
  const again = reuseByKey(
    kept,
    opened.items,
    [
      { key: "a", text: "one" },
      { key: "b", text: "two" },
    ],
    (item) => item.key,
    (item) => item.text,
  );
  expect(again.changed).toBe(false);
  expect(again.items).toBe(opened.items);
  expect(again.items[0]).toBe(first[0]);
  expect(again.items[1]).toBe(first[1]);
});

test("a new post prepended keeps the rows already on screen", () => {
  const kept = new Map<string, { key: string }>();
  const first = [{ key: "a" }, { key: "b" }];
  const opened = reuseByKey(
    kept,
    [],
    first,
    (item) => item.key,
    (item) => item.key,
  );
  const next = reuseByKey(
    kept,
    opened.items,
    [{ key: "c" }, { key: "a" }, { key: "b" }],
    (item) => item.key,
    (item) => item.key,
  );
  expect(next.changed).toBe(true);
  expect(next.items[0].key).toBe("c");
  expect(next.items[1]).toBe(first[0]);
  expect(next.items[2]).toBe(first[1]);
});

test("a signed image url does not count as a new post", () => {
  const post = {
    id: "1",
    timeMs: 10,
    unread: true,
    images: ["https://cdninstagram.com/a?token=1"],
    linkTitle: "",
    linkUrl: "",
    segments: [{ text: "hello" }],
    quote: null,
  };
  const later = { ...post, images: ["https://cdninstagram.com/a?token=2"] };
  expect(postFp(later)).toBe(postFp(post));
  const edited = { ...post, segments: [{ text: "hello!" }] };
  expect(postFp(edited)).not.toBe(postFp(post));
});
