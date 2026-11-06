import { Core } from "theysay/native";
import { Storage, signal } from "nsh";
import { postFp } from "./stable";
import { pushToast } from "./toast";

export const STORAGE_KEY = "theysay.profile.v1";
export const SESSION_KEY = "theysay.session.v1";

export interface Profile {
  version: number;
  focus: FocusId;
  x: Board;
  threads: Board;
  instagram: Board;
  weibo: Board;
  rss: Board;
  folders: Folder[];
  cache: unknown[];
  settings: Settings;
  sync: SyncPrefs;
}

export interface SyncPrefs {
  mode: string;
  url: string;
  token: string;
  pullOnOpen: boolean;
  lastSyncMs: number;
}

export interface Board {
  cards: CardState[];
}

export interface Folder {
  id: string;
  name: string;
}

export interface CardState {
  handle: string;
  label: string;
  posts: unknown[];
  seen: string[];
  lastFetchMs: number;
  lastAttemptMs?: number;
  note: string;
  /** Shared folder id. Empty means 未分类. */
  folder?: string;
  unread?: number;
}

export interface Settings {
  refreshSecs: number;
  muteKeywords: string[];
  theme: string;
  notify?: boolean;
  notifyX?: boolean;
  notifyThreads?: boolean;
  notifyInstagram?: boolean;
  notifyWeibo?: boolean;
  notifyRss?: boolean;
  home?: string;
  translateLang?: string;
  translateUrl?: string;
  translateToken?: string;
  translateModel?: string;
  briefingHour?: number;
  briefingAuto?: boolean;
}

export interface Envelope {
  ok: boolean;
  error: string;
  applied: boolean;
  profile: Profile;
}

export interface Segment {
  kind: string;
  text: string;
  symbol: string;
  tone: number;
}

export interface PostView {
  id: string;
  segments: Segment[];
  timeMs: number;
  url: string;
  images: string[];
  linkTitle: string;
  linkUrl: string;
  quote: { author: string; url: string; segments: Segment[] } | null;
  unread: boolean;
  starred?: boolean;
}

export interface CardView {
  ok: boolean;
  error: string;
  platform: string;
  handle: string;
  label: string;
  avatar: string;
  url: string;
  unread: number;
  note: string;
  lastFetchMs: number;
  posts: PostView[];
}

export type PlatformId = "x" | "threads" | "instagram" | "weibo" | "rss";
export type FocusId = PlatformId | "all";
export type SessionState = "unknown" | "in" | "out" | "blocked";

export interface ViewSlot {
  platform: string;
  index: number;
}

export interface UnreadTotals {
  x: number;
  threads: number;
  instagram: number;
  weibo: number;
  rss: number;
  all: number;
}

export const PLATFORMS: PlatformId[] = ["x", "threads", "instagram", "weibo"];

export const profile = signal<Profile>(blankProfile());
/** Bumps when the in-memory dashboard changes. Readers use this instead of reparsing posts. */
export const revision = signal(0);
/** Indexes into the board that stay mounted: the viewport, plus two rows each side. */
export const visibleSpan = signal({ start: 0, end: 6 });
export const ready = signal(false);
export const sessions = signal<Record<PlatformId, SessionState>>({
  x: "unknown",
  threads: "unknown",
  instagram: "unknown",
  weibo: "unknown",
  rss: "unknown",
});
/** Non-zero while a hidden home-page probe is open. */
export const probe = signal(0);
export const probePlatform = signal<PlatformId>("x");
export const loginOpen = signal(false);
export const loginFocus = signal<PlatformId>("x");
export const readerUrl = signal("");
export const settingsOpen = signal(false);
export const opmlOpen = signal(false);
export const manageOpen = signal(false);
/** Source picker for a new account or feed. Not the empty cell at the end of 看板. */
export const addOpen = signal(false);
export const starView = signal(false);
export const briefingOpen = signal(false);
/**
 * RSS browse filter. `""` is every feed, `"-"` is 未分类, otherwise a folder id.
 * It is not saved: switching chips does not rewrite the profile.
 */
/** `""` is 全部, `"-"` is 未分类, otherwise a shared folder id. Not saved. */
export const activeFolder = signal("");
/** Handle to scroll into view after a notification click. */
export const scrollHandle = signal("");

export function platformName(id: string) {
  if (id === "threads") return "Threads";
  if (id === "instagram") return "Instagram";
  if (id === "weibo") return "微博";
  if (id === "rss") return "RSS";
  return "X";
}

export function sessionOf(id: string): SessionState {
  if (id === "threads" || id === "instagram" || id === "weibo" || id === "rss" || id === "x") return sessions()[id];
  return "unknown";
}

export function loadSessions(saved: string | null) {
  if (!saved) return;
  if (saved === "in" || saved === "out" || saved === "blocked") {
    sessions.set({ ...sessions(), x: saved });
    return;
  }
  try {
    const parsed = JSON.parse(saved) as Partial<Record<PlatformId, SessionState>>;
    const next = { ...sessions() };
    for (const id of PLATFORMS) {
      const value = parsed[id];
      if (value === "in" || value === "out" || value === "blocked") next[id] = value;
    }
    sessions.set(next);
  } catch {
    /* keep the defaults */
  }
}

export function rememberSession(platform: PlatformId, next: Exclude<SessionState, "unknown">) {
  sessions.set({ ...sessions(), [platform]: next });
  void Storage.setItem(SESSION_KEY, JSON.stringify(sessions()));
}

export function openLogin(platform: PlatformId) {
  loginFocus.set(platform);
  loginOpen.set(true);
}

export function beginSessionProbe() {
  probePlatform.set("x");
  const next = Date.now() || 1;
  probe.set(next === probe() ? next + 1 : next);
}

export function advanceProbe() {
  const order = PLATFORMS;
  const index = order.indexOf(probePlatform());
  const next = order[index + 1];
  if (!next) {
    probe.set(0);
    return;
  }
  probePlatform.set(next);
  const id = Date.now() || 1;
  probe.set(id === probe() ? id + 1 : id);
}

export function notify(text: string) {
  pushToast(text);
}

export function applyEnvelope(raw: string): Envelope {
  const env = JSON.parse(raw) as Envelope;
  if (env.ok) profile.set(env.profile);
  return env;
}

/** Logs JS work that held the UI thread long enough to delay a paint. */
export function noteSlow(source: string, started: number) {
  const ms = Date.now() - started;
  if (ms >= 50) Core.noteSlow(source, ms);
}

export interface DashCard {
  handle: string;
  label: string;
  lastFetchMs: number;
  lastAttemptMs: number;
  note: string;
  folder?: string;
  unread?: number;
}

export interface Dash {
  ok: boolean;
  error: string;
  applied: boolean;
  revision: number;
  version: number;
  focus: string;
  settings: Settings;
  x: DashCard[];
  threads: DashCard[];
  instagram: DashCard[];
  weibo: DashCard[];
  rss: DashCard[];
  folders?: Folder[];
  sync?: SyncPrefs;
}

export interface CardClock {
  handle: string;
  lastFetchMs: number;
  lastAttemptMs: number;
  ids: string;
}

function blankProfile(): Profile {
  return {
    version: 1,
    focus: "all",
    x: { cards: [] },
    threads: { cards: [] },
    instagram: { cards: [] },
    weibo: { cards: [] },
    rss: { cards: [] },
    folders: [],
    cache: [],
    settings: {
      refreshSecs: 120,
      muteKeywords: [],
      theme: "system",
      notify: true,
      notifyX: true,
      notifyThreads: true,
      notifyInstagram: true,
      notifyWeibo: true,
      notifyRss: true,
      home: "timeline",
      translateLang: "zh",
      translateUrl: "",
      translateToken: "",
      translateModel: "gpt-4o-mini",
      briefingHour: 8,
      briefingAuto: true,
    },
    sync: { mode: "off", url: "", token: "", pullOnOpen: false, lastSyncMs: 0 },
  };
}

function cardsFrom(rows: DashCard[] | undefined): Board {
  return {
    cards: (rows ?? []).map((row) => ({
      handle: row.handle,
      label: row.label,
      posts: [],
      seen: [],
      lastFetchMs: row.lastFetchMs ?? 0,
      lastAttemptMs: row.lastAttemptMs ?? 0,
      note: row.note ?? "",
      folder: row.folder ?? "",
      unread: row.unread ?? 0,
    })),
  };
}

export function applyDash(raw: string): Dash {
  return applyParsed(JSON.parse(raw) as Dash);
}

export function applyParsed(env: Dash): Dash {
  if (!env.ok) return env;
  const focus =
    env.focus === "threads" || env.focus === "instagram" || env.focus === "weibo" || env.focus === "rss" || env.focus === "all"
      ? env.focus
      : "x";
  profile.set({
    version: env.version || 1,
    focus,
    x: cardsFrom(env.x),
    threads: cardsFrom(env.threads),
    instagram: cardsFrom(env.instagram),
    weibo: cardsFrom(env.weibo),
    rss: cardsFrom(env.rss),
    folders: env.folders ?? [],
    cache: [],
    settings: env.settings,
    sync: {
      mode: env.sync?.mode || "off",
      url: env.sync?.url ?? "",
      token: env.sync?.token ?? "",
      pullOnOpen: env.sync?.pullOnOpen === true,
      lastSyncMs: env.sync?.lastSyncMs ?? 0,
    },
  });
  revision.set(env.revision || 0);
  return env;
}

const presented = new Map<string, CardView>();

function reusePosts(prev: PostView[], next: PostView[]): PostView[] {
  const byId = new Map(prev.map((post) => [post.id, post]));
  let same = prev.length === next.length;
  const out = next.map((post, index) => {
    const old = byId.get(post.id);
    if (old && postFp(old) === postFp(post)) {
      if (prev[index] !== old) same = false;
      return old;
    }
    same = false;
    return post;
  });
  return same ? prev : out;
}

function copyChrome(onto: CardView, from: CardView) {
  onto.label = from.label;
  onto.unread = from.unread;
  onto.note = from.note;
  onto.lastFetchMs = from.lastFetchMs;
  onto.avatar = from.avatar;
  onto.handle = from.handle;
  onto.url = from.url;
  onto.error = from.error;
  onto.platform = from.platform;
}

export function presentCard(platform: string, index: number): CardView {
  revision();
  const key = `${platform}:${index}`;
  const started = Date.now();
  const fresh = JSON.parse(Core.presentCard(platform, index)) as CardView;
  noteSlow(`present-card ${key}`, started);
  const prev = presented.get(key);
  if (!prev) {
    presented.set(key, fresh);
    return fresh;
  }
  const posts = reusePosts(prev.posts, fresh.posts);
  if (posts === prev.posts) {
    copyChrome(prev, fresh);
    return prev;
  }
  const card = { ...fresh, posts };
  presented.set(key, card);
  return card;
}

let clockRev = -1;
const clocks = new Map<string, CardClock>();

export function cardClock(platform: string, index: number): CardClock {
  const rev = revision();
  if (rev !== clockRev) {
    clockRev = rev;
    clocks.clear();
  }
  const key = `${platform}:${index}`;
  const hit = clocks.get(key);
  if (hit) return hit;
  const started = Date.now();
  const clock = JSON.parse(Core.cardClock(platform, index)) as CardClock;
  noteSlow(`card-clock ${key}`, started);
  clocks.set(key, clock);
  return clock;
}

let slotRev = -1;
let slotCache: ViewSlot[] = [];

export function viewSlots(): ViewSlot[] {
  const rev = revision();
  const folder = activeFolder();
  if (rev !== slotRev) {
    const started = Date.now();
    const raw = JSON.parse(Core.viewSlots()) as ViewSlot[] | { ok?: boolean };
    noteSlow("view-slots", started);
    slotCache = Array.isArray(raw) ? raw : [];
    slotRev = rev;
  }
  if (!folder) return slotCache;
  return slotCache.filter((slot) => {
    const id = slot.platform === "threads" || slot.platform === "instagram" || slot.platform === "weibo" || slot.platform === "rss" ? slot.platform : "x";
    const card = profile()[id]?.cards[slot.index];
    if (!card) return false;
    if (folder === "-") return !card.folder;
    return card.folder === folder;
  });
}

export function unreadTotals(): UnreadTotals {
  revision();
  const started = Date.now();
  const totals = JSON.parse(Core.unreadTotals()) as UnreadTotals;
  noteSlow("unread-totals", started);
  return totals;
}

export function focusedUnread(): number {
  revision();
  const started = Date.now();
  const total = Number(Core.focusedUnread());
  noteSlow("focused-unread", started);
  return total;
}

export function focusedFreshMs(): number {
  revision();
  const started = Date.now();
  const ms = Number(Core.focusedFreshMs());
  noteSlow("focused-fresh", started);
  return ms;
}

export function focusName(id: string) {
  if (id === "all") return "全部";
  return platformName(id);
}

export function homeBoard() {
  return profile().settings.home === "board";
}
