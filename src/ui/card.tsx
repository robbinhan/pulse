import { Core } from "theysay/native";
import { Image, Scroll, Text, View, WebView, cleanup, openUrl, platform, type YNode } from "nsh";
import { Button, Input, Menu, MenuItem, Progress } from "nsh/base";
import { effect, signal } from "nsh";
import { Glyph } from "./chrome";
import { browseAgent, weiboResolveScript } from "./web";
import {
  applyParsed,
  cardClock,
  noteSlow,
  notify,
  openLogin,
  platformName,
  presentCard,
  profile,
  readerUrl,
  activeFolder,
  viewSlots,
  type CardView,
  type PlatformId,
  type PostView,
} from "./model";
import { slots } from "./scrapers";
import { postFp, reuseByKey } from "./stable";
import { RichText, clockLabel, groupPosts, relativeTime } from "./text";
import { PostTranslation, segmentText } from "./translate";
import { StarButton } from "./star";

/** Survives a card re-mount. A fetch must not open the list on the latest post again. */
const openedFeeds = new Set<string>();

type Place = { y: number; pinned: boolean; anchor: string; anchorOnScreen: number };

/** Last place the reader was in each account's post list. */
const places = new Map<string, Place>();

/** Post rows, so a refresh can put the same post back on screen. */
const postNodes = new Map<string, YNode>();

function feedKey(platform: string, handle: string) {
  return `${platform}:${handle}`;
}

export function Card(props: { cell: number }) {
  const draft = signal("");
  const formError = signal("");
  const editing = signal(false);
  const resolving = signal(false);
  const resolveUrl = signal("");
  const resolveQuery = signal("");
  const menuOpen = signal(false);
  /** Unread drawn before the profile is saved. `null` uses the saved count. */
  const optimisticUnread = signal<number | null>(null);
  const addAs = signal<PlatformId>("x");
  let seenSlot = "";
  let scroller!: YNode;

  // Read the focused list on every paint. Copying the slot once would keep
  // the first platform (X) after the sidebar selection changes.
  const slot = () => viewSlots()[props.cell];
  const adding = () => props.cell === viewSlots().length;
  const placed = () => !!slot()?.platform;
  const platform = (): PlatformId => {
    if (adding()) return addAs();
    const id = slot()?.platform;
    return id === "threads" || id === "instagram" || id === "weibo" || id === "rss" ? id : "x";
  };
  const index = () => (adding() ? (profile()[platform()]?.cards.length ?? 0) : (slot()?.index ?? 0));

  effect(() => {
    const id = platform();
    const at = index();
    const handle = profile()[id]?.cards[at]?.handle ?? "";
    const key = placed() ? `${id}:${at}:${handle}` : "";
    if (key !== seenSlot) {
      seenSlot = key;
      editing.set(false);
      menuOpen.set(false);
      optimisticUnread.set(null);
      formError.set("");
      draft.set(handle);
    }
  });

  let offsetY = 0;
  let contentH = 0;
  let seenSig = "";
  effect(() => {
    if (!placed() || !scroller) return;
    const clock = cardClock(platform(), index());
    if (!clock.handle || !clock.ids) return;
    const key = feedKey(platform(), clock.handle);
    if (openedFeeds.has(key)) return;
    openedFeeds.add(key);
    scroller.scrollToEnd({ animated: false });
  });
  effect(() => {
    if (!placed() || !scroller) return;
    const id = platform();
    const at = index();
    const clock = cardClock(id, at);
    if (!clock.handle) return;
    const sig = `${clock.lastFetchMs}|${clock.lastAttemptMs}|${clock.ids}`;
    if (sig === seenSig) return;
    const first = seenSig === "";
    seenSig = sig;
    const key = feedKey(id, clock.handle);
    const saved = places.get(key);
    const before = saved?.y ?? 0;
    const pinned = saved?.pinned ?? false;
    const anchor = saved?.anchor ?? "";
    const anchorOnScreen = saved?.anchorOnScreen ?? 0;
    // A card that scrolled off and came back is a new scroller at offset 0.
    // Put the saved place back. The first visit has no saved place yet.
    const revisit = first && openedFeeds.has(key) && !!saved && (pinned || before > 24);
    if (first && !revisit) return;
    const timer = setTimeout(() => {
      const now = offsetY;
      // A refresh of an open list must not scroll. Only a card that was
      // unmounted and came back needs its saved place, and only the bottom
      // follows new posts.
      if (revisit && pinned) {
        scroller.scrollToEnd({ animated: false });
        return;
      }
      if (revisit && before > 24) {
        void restoreFeed(scroller, key, before, anchor, anchorOnScreen);
        return;
      }
      if (!revisit) Core.noteScroll(key, before, now);
    }, 0);
    cleanup(() => clearTimeout(timer));
  });

  const view = () => (placed() ? presentCard(platform(), index()) : emptyView());
  const loading = () => placed() && slots().some((item) => item.platform === platform() && item.index === index());
  const needsLogin = () => view().note.indexOf("需要登录") >= 0;

  let weiboSaved = false;

  function failAdd(message: string) {
    const text = message || "没找到这个微博账号";
    formError.set(text);
    notify(text);
    resolving.set(false);
    weiboSaved = false;
  }

  function saveWeibo(uid: string, name: string, avatar: string, query: string, via: string) {
    if (weiboSaved) return;
    weiboSaved = true;
    resolving.set(false);
    const started = Date.now();
    const env = applyParsed(JSON.parse(Core.addWeibo(index(), uid, name, avatar, query, via)));
    noteSlow("add-weibo", started);
    if (!env.ok) {
      failAdd(env.error || "没有加上");
      return;
    }
    formError.set("");
    editing.set(false);
    menuOpen.set(false);
  }

  function commitHandle() {
    const id = platform();
    const value = draft();
    Core.noteCommit(id, value);
    if (id !== "weibo") {
      const wasAdding = adding();
      const started = Date.now();
      const env = applyParsed(JSON.parse(Core.setHandle(id, index(), value)));
      noteSlow("set-handle", started);
      if (!env.ok) {
        formError.set(env.error || "没有加上");
        notify(env.error || "没有加上");
        return;
      }
      formError.set("");
      editing.set(false);
      menuOpen.set(false);
      const want = activeFolder();
      if (wasAdding && want && want !== "-") {
        const cards = profile()[id].cards;
        const handle = cards[cards.length - 1]?.handle;
        if (handle) applyParsed(JSON.parse(Core.moveCardFolder(id, handle, want)));
      }
      return;
    }
    if (resolving()) return;
    let query: { ok?: boolean; needsResolve?: boolean; uid?: string; q?: string; via?: string; name?: string; avatar?: string; error?: string };
    try {
      query = JSON.parse(Core.weiboQuery(value));
    } catch {
      failAdd("没找到这个微博账号");
      return;
    }
    if (!query.ok) {
      failAdd(query.error || "没找到这个微博账号");
      return;
    }
    if (query.needsResolve) {
      weiboSaved = false;
      formError.set("");
      resolveQuery.set(query.q || "");
      resolveUrl.set(`https://m.weibo.cn/n/${encodeURIComponent(query.q || "")}`);
      resolving.set(true);
      return;
    }
    saveWeibo(query.uid || "", query.name || "", query.avatar || "", query.q || value, query.via || "url");
  }

  effect(() => {
    if (!resolving()) return;
    const timer = setTimeout(() => {
      if (resolving() && !weiboSaved) failAdd("没找到这个微博账号");
    }, 12000);
    cleanup(() => clearTimeout(timer));
  });

  function markRead() {
    const started = Date.now();
    const id = platform();
    const at = index();
    // Hide the badge before any profile JSON. The save runs on the task
    // after the next frame, so this paint is not the stringify.
    optimisticUnread.set(0);
    requestAnimationFrame(() => {
      Core.noteClick("mark-read-frame", Date.now() - started);
      setTimeout(() => {
        const saving = Date.now();
        const env = applyParsed(JSON.parse(Core.markRead(id, at)));
        noteSlow("mark-read", saving);
        if (!env.ok) optimisticUnread.set(null);
        requestAnimationFrame(() => optimisticUnread.set(null));
      }, 0);
    });
  }

  function shift(direction: number) {
    const started = Date.now();
    const env = applyParsed(JSON.parse(Core.moveCard(platform(), index(), direction)));
    noteSlow("move-card", started);
    if (!env.ok) return;
  }

  function removeAccount() {
    editing.set(false);
    draft.set("");
    const started = Date.now();
    const env = applyParsed(JSON.parse(Core.setHandle(platform(), index(), "")));
    noteSlow("remove-account", started);
    if (!env.ok) return;
  }

  function assignFolder(id: string) {
    menuOpen.set(false);
    const env = applyParsed(JSON.parse(Core.moveCardFolder(platform(), view().handle, id)));
    if (!env.ok) notify(env.error || "没有移动");
  }

  function startEdit() {
    draft.set(view().handle);
    formError.set("");
    menuOpen.set(false);
    editing.set(true);
  }

  const unreadCount = () => {
    const pending = optimisticUnread();
    if (pending !== null) return pending;
    return view().unread;
  };

  return (
    <View
      class="relative min-h-0 min-w-0 flex-1 overflow-hidden rounded-[16px] border border-border bg-card shadow-sm"
      style={{ flexDirection: "column", minHeight: 0 }}
    >
      <If when={adding() && !editing()}>
        <View class="min-h-0 flex-1 items-center justify-center gap-3 px-4">
          <Text class="text-sm font-medium text-card-foreground">添加账号</Text>
          <Text class="text-center text-xs text-muted-foreground">选一个来源。它会进当前文件夹。</Text>
          <View class="flex-row flex-wrap justify-center gap-2">
            <AddSource id="x" label="添加 X" onPick={(id) => { addAs.set(id); editing.set(true); }} />
            <AddSource id="threads" label="添加 Threads" onPick={(id) => { addAs.set(id); editing.set(true); }} />
            <AddSource id="instagram" label="添加 Instagram" onPick={(id) => { addAs.set(id); editing.set(true); }} />
            <AddSource id="weibo" label="添加微博账号" onPick={(id) => { addAs.set(id); editing.set(true); }} />
            <AddSource id="rss" label="添加 RSS" onPick={(id) => { addAs.set(id); editing.set(true); }} />
          </View>
        </View>
      </If>
      <If when={placed() && !view().handle && !editing()}>
        <Button
          onPress={startEdit}
          accessibilityLabel={`添加${platformName(platform())}账号`}
          class="min-h-0 flex-1 items-center justify-center gap-2 border border-dashed border-border bg-card active:opacity-80"
        >
          <View class="h-10 w-10 items-center justify-center rounded-[12px] bg-muted">
            <Text class="text-lg text-muted-foreground">＋</Text>
          </View>
          <Text class="text-sm font-medium text-card-foreground">添加账号</Text>
          <Text class="text-xs text-muted-foreground">只加到{platformName(platform())}</Text>
        </Button>
      </If>
      <If when={placed() && (!!view().handle || editing())}>
        <View class="flex-row items-center gap-3 border-b border-border px-4 py-3">
          <If when={!!view().avatar}>
            <Avatar src={view().avatar} platform={platform()} />
          </If>
          <If when={!view().avatar}>
            <Glyph platform={view().platform || platform()} size={36} />
          </If>
          <View class="min-w-0 flex-1">
            <Text class="text-[15px] font-semibold text-card-foreground" numberOfLines={1}>
              {title(view())}
            </Text>
            <Text class="text-xs text-muted-foreground" numberOfLines={1}>
              {view().handle ? (platform() === "rss" ? hostOf(view().handle) : `@${view().handle}`) : "新账号"}
              {view().lastFetchMs ? ` · ${relativeTime(view().lastFetchMs)}` : ""}
            </Text>
          </View>
          <If when={unreadCount() > 0}>
            <Button
              onPress={markRead}
              accessibilityLabel="把这个格子标为已读"
              class="min-h-7 min-w-7 items-center justify-center rounded-full bg-primary px-2 active:opacity-60"
            >
              <Text class="text-xs font-semibold text-primary-foreground">{unreadCount()}</Text>
            </Button>
          </If>
          <If when={!!view().handle}>
            <Menu
              open={menuOpen()}
              onOpenChange={(open, reason) => {
                menuOpen.set(open);
                Core.noteClick(`menu ${open ? "open" : "close"} ${reason}`, 0);
              }}
              placement="bottom"
              align="end"
              offset={4}
              popupClass={menuPopupClass()}
              backdropClass="bg-black/40"
              trigger={
                <View
                  accessibilityLabel="账号菜单"
                  class={
                    onPhone()
                      ? "h-11 w-11 items-center justify-center rounded-[12px] active:opacity-60"
                      : "h-8 w-8 items-center justify-center rounded-[8px] hover:bg-accent active:opacity-60"
                  }
                >
                  <Text class="text-base text-muted-foreground">⋯</Text>
                </View>
              }
            >
              <MenuItem onSelect={startEdit} class={menuRow()}>
                <Text class="text-sm text-popover-foreground">编辑账号</Text>
              </MenuItem>
              <MenuItem onSelect={() => assignFolder("")} class={menuRow()}>
                <Text class="text-sm text-popover-foreground">移到未分类</Text>
              </MenuItem>
              {(profile().folders ?? []).map((folder) => (
                <MenuItem onSelect={() => assignFolder(folder.id)} class={menuRow()}>
                  <Text class="text-sm text-popover-foreground" numberOfLines={1}>
                    移到 {folder.name}
                  </Text>
                </MenuItem>
              ))}
              <If when={profile().focus !== "all"}>
                <MenuItem onSelect={() => shift(-1)} class={menuRow()}>
                  <Text class="text-sm text-popover-foreground">上移</Text>
                </MenuItem>
                <MenuItem onSelect={() => shift(1)} class={menuRow()}>
                  <Text class="text-sm text-popover-foreground">下移</Text>
                </MenuItem>
              </If>
              <MenuItem onSelect={removeAccount} class={menuRow()}>
                <Text class="text-sm text-destructive">移除</Text>
              </MenuItem>
            </Menu>
          </If>
        </View>
      </If>
      <If when={editing()}>
        <View class="gap-2 border-b border-border px-4 py-3">
          <Input
            value={draft()}
            onChange={(text) => draft.set(text)}
            onSubmit={commitHandle}
            placeholder={
              platform() === "weibo"
                ? "昵称、数字 uid，或主页链接"
                : platform() === "rss"
                  ? "订阅地址，例如 https://example.com/feed.xml"
                  : "@handle"
            }
            autoCapitalize="none"
            autoCorrect={false}
            returnKeyType="go"
            class={
              onPhone()
                ? "min-h-11 rounded-[12px] bg-muted px-3 text-[16px] text-foreground placeholder:text-muted-foreground"
                : "rounded-[10px] bg-muted px-3 py-1.5 text-sm text-foreground placeholder:text-muted-foreground"
            }
          />
          <View class="flex-row items-center gap-3">
            <Button
              onPress={commitHandle}
              class={
                onPhone()
                  ? "min-h-11 justify-center rounded-[12px] bg-primary px-4 active:opacity-80"
                  : "rounded-[8px] bg-primary px-3 py-1 active:opacity-80"
              }
            >
              <Text class={onPhone() ? "text-sm font-semibold text-primary-foreground" : "text-xs font-semibold text-primary-foreground"}>
                完成
              </Text>
            </Button>
            <Button
              onPress={() => {
                editing.set(false);
                formError.set("");
                draft.set(view().handle);
              }}
              class="px-2 py-1 active:opacity-70"
            >
              <Text class="text-xs text-muted-foreground">取消</Text>
            </Button>
          </View>
          <If when={resolving()}>
            <Progress
              indeterminate
              accessibilityLabel="正在查找这个微博账号"
              trackClass="h-1.5 overflow-hidden rounded-full bg-muted"
              indicatorClass="h-full w-1/3 rounded-full bg-primary animate-pulse"
            >
              <Text class="text-xs text-muted-foreground">正在查找这个微博账号…</Text>
            </Progress>
            <WebView
              src={resolveUrl()}
              userAgent={browseAgent("weibo")}
              injectedJavaScript={weiboResolveScript(resolveQuery())}
              pointerEvents="none"
              class="pointer-events-none"
              onMessage={(event) => {
                let body: { type?: string; via?: string; raw?: string; uid?: string };
                try {
                  body = JSON.parse(event.data);
                } catch {
                  return;
                }
                if (body.type !== "weibo-resolve") return;
                if (body.via === "profile") {
                  let hint: { name?: string; avatar?: string } = {};
                  try {
                    hint = JSON.parse(Core.weiboProfile(body.raw || ""));
                  } catch {
                    hint = {};
                  }
                  if (body.uid) saveWeibo(body.uid, hint.name || resolveQuery(), hint.avatar || "", resolveQuery(), "profile");
                  else if (!weiboSaved) failAdd("没找到这个微博账号");
                  return;
                }
                if (body.via === "redirect") {
                  if (body.uid) saveWeibo(body.uid, resolveQuery(), "", resolveQuery(), "redirect");
                  return;
                }
                let hit: { ok?: boolean; uid?: string; name?: string; avatar?: string; error?: string };
                try {
                  hit = JSON.parse(Core.weiboMatch(resolveQuery(), body.raw || ""));
                } catch {
                  failAdd("没找到这个微博账号");
                  return;
                }
                if (hit.ok && hit.uid) saveWeibo(hit.uid, hit.name || resolveQuery(), hit.avatar || "", resolveQuery(), "search");
                else if (!weiboSaved) failAdd(hit.error || "没找到这个微博账号");
              }}
              style={{ position: "absolute", left: -4200, top: 0, width: 480, height: 900, opacity: 0 }}
            />
          </If>
          <If when={!!formError()}>
            <Text class="text-xs text-destructive">{formError()}</Text>
          </If>
        </View>
      </If>
      <If when={placed() && !!view().handle}>
        <View style={{ flex: 1, minHeight: 0 }}>
        <Scroll
          ref={scroller}
          zoomable={false}
          scrollable
          style={{ flex: 1, minHeight: 0 }}
          class="px-3 pb-2"
          onScroll={(event) => {
            const offset = event.offset.y;
            const view = event.size.height;
            const content = event.contentSize.height;
            // A refresh that collapses the content for a frame reports offset 0.
            // Leave the place the reader is actually at.
            if (content < 1 && offsetY > 24) return;
            const next = Core.scrollOffset(offsetY, offset, view, contentH, content);
            contentH = content;
            offsetY = next >= 0 ? next : offset;
            const handle = placed() ? presentCard(platform(), index()).handle : "";
            if (handle && scroller) {
              const pinned = content > 0 && offsetY + view >= content - 24;
              const key = feedKey(platform(), handle);
              const prev = places.get(key);
              places.set(key, {
                y: offsetY,
                pinned,
                anchor: prev?.anchor ?? "",
                anchorOnScreen: prev?.anchorOnScreen ?? 0,
              });
              void rememberAnchor(key, scroller);
            }
            if (next >= 0 && scroller && Math.abs(next - offset) > 1) {
              scroller.scrollTo({ y: next, animated: false });
            }
          }}
        >
          <If when={loading() && view().posts.length === 0}>
            <Skeleton />
          </If>
          <If when={needsLogin() && !loading()}>
            <LoginEmpty platform={platform()} note={view().note} />
          </If>
          <If when={view().posts.length === 0 && !loading() && !needsLogin()}>
            <View class="items-center px-4 py-10">
              <Text class="text-sm font-medium text-card-foreground">还没有帖子</Text>
              <Text class="mt-1 text-center text-xs leading-5 text-muted-foreground">{view().note || "新的更新会出现在这里。"}</Text>
            </View>
          </If>
          <If when={view().posts.length > 0}>
            <Feed posts={view().posts} platform={platform()} feed={feedKey(platform(), view().handle)} />
          </If>
        </Scroll>
        </View>
      </If>
    </View>
  );
}

function emptyView(): CardView {
  return {
    ok: true,
    error: "",
    platform: "",
    handle: "",
    label: "",
    avatar: "",
    url: "",
    unread: 0,
    note: "",
    lastFetchMs: 0,
    posts: [],
  };
}

function AddSource(props: { id: PlatformId; label: string; onPick: (id: PlatformId) => void }) {
  return (
    <Button
      onPress={() => props.onPick(props.id)}
      accessibilityLabel={props.label}
      class="rounded-full bg-secondary px-3 py-1.5 active:opacity-80"
    >
      <Text class="text-xs font-medium text-secondary-foreground">{props.label}</Text>
    </Button>
  );
}

export function MixPlaceholder() {
  return (
    <View class="min-h-0 min-w-0 flex-1 items-center justify-center rounded-[16px] border border-dashed border-border bg-card px-4">
      <Text class="text-center text-sm text-muted-foreground">在 X、Threads、Instagram、微博或 RSS 里添加</Text>
    </View>
  );
}

function onPhone() {
  return platform.os === "ios";
}

function menuPopupClass() {
  return platform.idiom === "desktop"
    ? "min-w-36 rounded-[12px] border border-border bg-popover p-1 shadow-md"
    : "rounded-t-[16px] border border-border bg-popover p-2 shadow-md";
}

function menuRow() {
  return onPhone()
    ? "min-h-11 justify-center rounded-[10px] px-3 active:bg-accent"
    : "rounded-[8px] px-3 py-2 hover:bg-accent";
}

function title(view: CardView) {
  if (view.label) return view.label;
  if (view.platform === "rss" && view.handle) return hostOf(view.handle);
  if (view.handle) return `@${view.handle}`;
  return "添加账号";
}

function hostOf(url: string) {
  const rest = url.replace(/^https?:\/\//, "");
  const host = rest.split(/[/?#]/)[0] ?? rest;
  return host.replace(/^www\./, "") || url;
}

function Skeleton() {
  return (
    <View class="px-2 py-6">
      <Progress
        indeterminate
        accessibilityLabel="正在读取帖子"
        trackClass="h-1.5 overflow-hidden rounded-full bg-muted"
        indicatorClass="h-full w-1/3 rounded-full bg-primary animate-pulse"
      >
        <Text class="text-xs text-muted-foreground">正在读取…</Text>
      </Progress>
    </View>
  );
}

function LoginEmpty(props: { platform: string; note: string }) {
  const id: PlatformId =
    props.platform === "threads" || props.platform === "instagram" || props.platform === "weibo" || props.platform === "rss"
      ? props.platform
      : "x";
  return (
    <View class="items-center px-4 py-10">
      <Glyph platform={id} size={40} />
      <Text class="mt-3 text-center text-sm font-medium text-card-foreground">{props.note}</Text>
      <Button onPress={() => openLogin(id)} class="mt-4 rounded-[10px] bg-primary px-4 py-2 active:opacity-80">
        <Text class="text-sm font-semibold text-primary-foreground">去登录</Text>
      </Button>
    </View>
  );
}

type FeedLine = { kind: "day"; label: string } | { kind: "post"; post: PostView };

function lineKey(line: FeedLine) {
  return line.kind === "day" ? `d:${line.label}` : `p:${line.post.id}`;
}

function lineFp(line: FeedLine) {
  return line.kind === "day" ? lineKey(line) : `p:${postFp(line.post)}`;
}

function Feed(props: { posts: PostView[]; platform: string; feed: string }) {
  const kept = new Map<string, FeedLine>();
  let order: FeedLine[] = [];
  const rows = () => {
    const adopted = reuseByKey(kept, order, groupPosts(props.posts), lineKey, lineFp);
    order = adopted.items;
    return adopted.items;
  };
  return (
    <View class="pt-1">
      <Each of={rows()} key={(row) => lineKey(row)}>
        {(row) => <FeedRow row={row()} platform={props.platform} feed={props.feed} />}
      </Each>
    </View>
  );
}

function FeedRow(props: {
  row: { kind: "day"; label: string } | { kind: "post"; post: PostView };
  platform: string;
  feed: string;
}) {
  let node!: YNode;
  effect(() => {
    if (props.row.kind !== "post" || !node) return;
    const id = `${props.feed}:${props.row.post.id}`;
    postNodes.set(id, node);
    cleanup(() => {
      if (postNodes.get(id) === node) postNodes.delete(id);
    });
  });
  if (props.row.kind === "day") {
    return (
      <View class="my-1.5 items-center">
        <Text class="text-xs font-medium text-muted-foreground">{props.row.label}</Text>
      </View>
    );
  }
  return (
    <View ref={node}>
      <Bubble post={props.row.post} platform={props.platform} />
    </View>
  );
}

const anchorAt = new Map<string, number>();

async function rememberAnchor(key: string, scroller: YNode) {
  const now = Date.now();
  if (now - (anchorAt.get(key) ?? 0) < 250) return;
  anchorAt.set(key, now);
  const scrollBox = await scroller.measure();
  if (!scrollBox) return;
  let anchor = "";
  let anchorOnScreen = 0;
  for (const [id, node] of postNodes) {
    if (!id.startsWith(`${key}:`)) continue;
    const box = await node.measure();
    if (!box) continue;
    const onScreen = box.pagePosition.y - scrollBox.pagePosition.y;
    if (onScreen >= -8) {
      anchor = id.slice(key.length + 1);
      anchorOnScreen = onScreen;
      break;
    }
  }
  const prev = places.get(key);
  if (!prev || !anchor) return;
  places.set(key, { ...prev, anchor, anchorOnScreen });
}

async function restoreFeed(
  scroller: YNode,
  key: string,
  before: number,
  anchor: string,
  anchorOnScreen: number,
) {
  let after = before;
  const node = anchor ? postNodes.get(`${key}:${anchor}`) : undefined;
  if (node) {
    const postBox = await node.measure();
    const scrollBox = await scroller.measure();
    if (postBox && scrollBox) {
      const onScreen = postBox.pagePosition.y - scrollBox.pagePosition.y;
      after = Math.max(0, before + (onScreen - anchorOnScreen));
    }
  }
  scroller.scrollTo({ y: after, animated: false });
  Core.noteScroll(key, before, after);
}

function Avatar(props: { src: string; platform: string }) {
  const local = signal("");
  effect(() => {
    const src = props.src;
    let alive = true;
    function pull() {
      void Core.loadMedia(src, props.platform).then((uri) => {
        if (alive && uri) local.set(uri);
      });
    }
    pull();
    const later = setTimeout(pull, 1600);
    cleanup(() => {
      alive = false;
      clearTimeout(later);
    });
  });
  return (
    <Image
      src={local() || props.src}
      resizeMode="cover"
      style={{ width: 36, height: 36, borderRadius: 18 }}
    />
  );
}

function PhotoRow(props: { images: string[]; platform: string }) {
  const many = props.images.length > 1;
  return (
    <View class={many ? "mt-2 flex-row gap-2" : "mt-2"}>
      {props.images.map((src) => (
        <View
          class="overflow-hidden rounded-[12px] bg-muted"
          style={many ? { flex: 1, height: 148, minWidth: 0 } : { width: "100%", height: 196 }}
        >
          <PostImage src={src} platform={props.platform} tall={!many} />
        </View>
      ))}
    </View>
  );
}

function PostImage(props: { src: string; platform: string; tall: boolean }) {
  const local = signal("");
  effect(() => {
    const src = props.src;
    if (!src || src.indexOf("data:") === 0) {
      local.set(src);
      return;
    }
    let alive = true;
    function pull() {
      void Core.loadMedia(src, props.platform).then((uri) => {
        if (alive && uri) local.set(uri);
      });
    }
    pull();
    const later = setTimeout(pull, 1600);
    cleanup(() => {
      alive = false;
      clearTimeout(later);
    });
  });
  const height = props.tall ? 196 : 148;
  return (
    <Image
      src={local() || props.src}
      resizeMode="cover"
      onError={() => {
        if (!local()) local.set("");
      }}
      style={{ width: "100%", height, borderRadius: 12 }}
    />
  );
}

function Bubble(props: { post: PostView; platform: string }) {
  function openPost() {
    const post = props.post;
    if (!post.url) {
      notify("这条帖子没有链接");
      return;
    }
    readerUrl.set(post.url);
  }
  return (
    <View class="rounded-[12px] px-2 py-1.5 hover:bg-accent">
      <View selectable>
        <View
          class={
            props.post.unread
              ? "mb-1.5 h-1 w-6 rounded-full bg-primary"
              : "mb-1.5 h-1 w-6 rounded-full bg-transparent"
          }
        />
        <If when={props.platform === "rss" && !!props.post.linkTitle}>
          <Text selectable class="mb-1 text-[15px] font-semibold leading-5 text-card-foreground">
            {props.post.linkTitle}
          </Text>
        </If>
        <RichText segments={props.post.segments} />
        <If when={!!props.post.quote}>
          <View class="mt-2 rounded-[12px] border border-border bg-background px-3 py-2">
            <If when={!!props.post.quote?.author}>
              <Text selectable class="mb-1 text-xs font-medium text-muted-foreground">
                @{props.post.quote?.author}
              </Text>
            </If>
            <RichText segments={props.post.quote?.segments ?? []} dim />
          </View>
        </If>
      </View>
      <If when={props.post.images.length > 0}>
        <Button onPress={openPost} accessibilityLabel="打开图片" class="active:opacity-90">
          <PhotoRow images={props.post.images} platform={props.platform} />
        </Button>
      </If>
      <View class="mt-1 flex-row items-center gap-3">
        <Button onPress={openPost} accessibilityLabel="打开帖子" class="active:opacity-70">
          <Text class="text-xs font-medium text-primary">打开</Text>
        </Button>
        <StarButton platform={props.platform} id={props.post.id} on={!!props.post.starred} />
      </View>
      <PostTranslation
        id={`${props.platform}:${props.post.id}`}
        title={props.platform === "rss" ? props.post.linkTitle : ""}
        body={segmentText(props.post.segments)}
        quote={segmentText(props.post.quote?.segments)}
      />
      <View class="mt-1 flex-row items-center gap-3">
        <Text class="text-xs text-muted-foreground">{relativeTime(props.post.timeMs) || clockLabel(props.post.timeMs)}</Text>
        <If when={!!props.post.linkUrl}>
          <Button onPress={() => void openUrl(props.post.linkUrl)} class="min-w-0 flex-1 items-start active:opacity-70">
            <Text class="text-xs text-primary" numberOfLines={1}>
              {props.post.linkTitle || props.post.linkUrl}
            </Text>
          </Button>
        </If>
      </View>
    </View>
  );
}
