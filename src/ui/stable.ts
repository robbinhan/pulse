/**
 * NSH diffs `List`'s `each` and `Each`'s `of` by object identity
 * (`JS_IsStrictEqual` in `nsh-js` `splice_run` / `each_run`), not by the key
 * string. A fresh `JSON.parse` on every revision therefore splices every
 * row, sets its measured height back to NaN, and the scrollbar moves while
 * those heights are filled in again. Callers keep the previous object when
 * the fingerprint matches so a refresh that did not change the post does
 * not touch the open list.
 */
export function reuseByKey<T>(
  kept: Map<string, T>,
  order: T[],
  next: T[],
  keyOf: (item: T) => string,
  fpOf: (item: T) => string,
): { items: T[]; changed: boolean } {
  const live = new Set<string>();
  let changed = next.length !== order.length;
  const items = next.map((item, index) => {
    const key = keyOf(item);
    live.add(key);
    const prev = kept.get(key);
    const chosen = prev && fpOf(prev) === fpOf(item) ? prev : item;
    if (chosen !== prev) kept.set(key, chosen);
    if (order[index] !== chosen) changed = true;
    return chosen;
  });
  if (kept.size !== live.size) {
    changed = true;
    for (const key of kept.keys()) if (!live.has(key)) kept.delete(key);
  }
  if (!changed) return { items: order, changed: false };
  return { items, changed: true };
}

export function textHash(text: string): number {
  let hash = text.length | 0;
  const step = Math.max(1, Math.floor(text.length / 12));
  for (let i = 0; i < text.length; i += step) hash = (Math.imul(hash, 33) + text.charCodeAt(i)) | 0;
  return hash;
}

/** Identity of a bubble. Image URLs are left out: signed CDN links change every fetch and must not rebuild the row. */
export function postFp(post: {
  id: string;
  timeMs: number;
  unread: boolean;
  starred?: boolean;
  images: readonly unknown[];
  linkTitle: string;
  linkUrl: string;
  segments: readonly { text: string }[];
  quote: { author: string; segments: readonly { text: string }[] } | null;
}): string {
  let hash = post.segments.length | 0;
  for (const part of post.segments) hash = (Math.imul(hash, 33) + textHash(part.text)) | 0;
  const quote = post.quote;
  if (quote) {
    hash = (Math.imul(hash, 33) + textHash(quote.author)) | 0;
    for (const part of quote.segments) hash = (Math.imul(hash, 33) + textHash(part.text)) | 0;
  }
  return `${post.id}|${post.timeMs}|${post.unread ? 1 : 0}|${post.starred ? 1 : 0}|${post.images.length}|${textHash(post.linkTitle)}|${textHash(post.linkUrl)}|${hash}`;
}
