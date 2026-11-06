import { Core } from "theysay/native";
import { signal, untracked } from "nsh";
import { applyParsed, noteSlow, profile, ready, rememberSession, visibleSpan, type Dash, type PlatformId } from "./model";

const rssInflight = new Set<string>();

export interface Slot {
  index: number;
  attempt: number;
  started: number;
  handle: string;
  url: string;
  platform: PlatformId;
  terminal: boolean;
}

export interface FetchJob {
  platform: string;
  index: number;
}

/** Hidden profile pages. At most `Core.maxScrapers()` of them exist. */
export const slots = signal<Slot[]>([]);
export const pauseScrapers = signal(false);

export function slotKey(slot: { platform: string; index: number }) {
  return `${slot.platform}:${slot.index}`;
}

export function scraperToken(slot: Slot) {
  return `c${slot.platform}${slot.index}a${slot.attempt}`;
}

export function startScraperLoop() {
  const timer = setInterval(() => {
    untracked(tick);
  }, 500);
  return () => clearInterval(timer);
}

let planning = false;

function tick() {
  const started = Date.now();
  if (!ready() || planning) return;
  if (pauseScrapers()) {
    if (slots().length) slots.set([]);
    return;
  }
  const now = Date.now();
  const live = slots();
  const keep: Slot[] = [];
  let finished = false;
  for (const slot of live) {
    // X draws the latest posts a few seconds after navigation. Keep that
    // page up long enough for the late scrape. Closing the view is not a
    // successful parse: lastFetchMs moves only when posts were stored.
    const dwell = Math.max(12000, Core.scraperDwellMs());
    if (!(slot.terminal || now - slot.started >= dwell)) {
      keep.push(slot);
      continue;
    }
    finished = true;
    const step = Date.now();
    Core.finishFetch(slot.platform, slot.index);
    noteSlow(`finish-fetch ${slot.platform}:${slot.index}`, step);
  }
  noteSlow("scraper-finish", started);
  // Opening the next pages waits for a frame, so a badge click can paint
  // before another full profile pass.
  planning = true;
  const dirty = finished;
  requestAnimationFrame(() => {
    setTimeout(() => {
      planning = false;
      untracked(() => planFetches(keep, now, dirty));
    }, 0);
  });
}

function planFetches(keep: Slot[], now: number, dirty: boolean) {
  if (!ready() || pauseScrapers()) {
    if (slots().length) slots.set([]);
    return;
  }
  const started = Date.now();
  const busy = keep.map(slotKey).concat([...rssInflight]);
  const span = visibleSpan();
  const plan = JSON.parse(Core.nextFetches(now, JSON.stringify(busy), span.start, span.end)) as {
    ok: boolean;
    jobs: FetchJob[];
  };
  noteSlow("next-fetches", started);
  const startedSlots: Slot[] = [];
  if (plan.ok) {
    for (const job of plan.jobs ?? []) {
      const platform: PlatformId =
        job.platform === "threads" || job.platform === "instagram" || job.platform === "weibo" || job.platform === "rss"
          ? job.platform
          : "x";
      if (platform === "rss") {
        void fetchRss(job.index);
        continue;
      }
      if (busy.includes(slotKey({ platform, index: job.index }))) continue;
      const handle = profile()[platform]?.cards[job.index]?.handle ?? "";
      if (!handle) continue;
      const viewStarted = Date.now();
      const url = Core.profileUrl(platform, job.index);
      noteSlow(`fetch-url ${platform}:${job.index}`, viewStarted);
      if (!url) continue;
      startedSlots.push({
        index: job.index,
        attempt: now + job.index,
        started: now,
        handle,
        url: freshUrl(url, now + job.index),
        platform,
        terminal: false,
      });
    }
  }
  if (dirty || startedSlots.length) slots.set(keep.concat(startedSlots));
}

/** A new query forces the web view to navigate. The same profile URL on a
 * reused X document keeps yesterday's SPA timeline. */
async function fetchRss(index: number) {
  const key = `rss:${index}`;
  if (rssInflight.has(key)) return;
  const url = Core.profileUrl("rss", index);
  if (!url) return;
  rssInflight.add(key);
  try {
    const response = await fetch(url, {
      headers: { Accept: "application/rss+xml, application/atom+xml, application/xml, text/xml, */*" },
    });
    const xml = response.ok ? await response.text() : "";
    const env = JSON.parse(Core.ingestRss(index, xml)) as Dash;
    if (env.ok) applyParsed(env);
  } catch {
    const env = JSON.parse(Core.ingestRss(index, "")) as Dash;
    if (env.ok) applyParsed(env);
  } finally {
    rssInflight.delete(key);
  }
}

function freshUrl(url: string, nonce: number) {
  const hashAt = url.indexOf("#");
  const base = hashAt >= 0 ? url.slice(0, hashAt) : url;
  const hash = hashAt >= 0 ? url.slice(hashAt) : "";
  const joiner = base.indexOf("?") >= 0 ? "&" : "?";
  return `${base}${joiner}theysay=${nonce}${hash}`;
}

export function onScraperMessage(platform: string, index: number, data: string) {
  let body: { token?: string; type?: string; state?: string; posts?: { images?: string[] }[] };
  try {
    body = JSON.parse(data);
  } catch {
    return;
  }
  const slot = slots().find((item) => item.platform === platform && item.index === index);
  if (!slot || body.token !== scraperToken(slot)) return;
  const started = Date.now();
  const env = JSON.parse(Core.ingest(platform, index, data)) as Dash;
  noteSlow(`ingest ${platform}:${index} ${data.length}`, started);
  const id: PlatformId = slot.platform;
  // Public posts and a guest Weibo/Instagram API leave the saved status alone.
  const login = Core.sessionFromScrape(data);
  if (login === "in" || login === "out" || login === "blocked") rememberSession(id, login);
  if (!env.ok || !env.applied) return;
  applyParsed(env);
  prefetch(body, platform);
  const early =
    ((body.state === "signed-out" && id !== "weibo") || body.state === "blocked") ||
    ((id === "instagram" || id === "weibo") &&
      (body.type === "ig-api" || body.type === "weibo-api" || (body.posts?.length ?? 0) > 0));
  if (early) {
    slots.set(slots().map((item) => (item.platform === platform && item.index === index ? { ...item, terminal: true } : item)));
  }
}

function prefetch(body: { posts?: { images?: string[] }[] }, platform: string) {
  const urls: string[] = [];
  for (const post of body.posts ?? []) {
    for (const image of post.images ?? []) {
      if (urls.length === 8) break;
      if (urls.indexOf(image) === -1) urls.push(image);
    }
  }
  for (const url of urls) void Core.loadMedia(url, platform);
}
