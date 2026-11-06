import { Core } from "theysay/native";
import { Clipboard, List, Scroll, Text, View, WebView, cleanup, effect, openUrl, platform, setColorScheme, signal, windowSize, type YNode } from "nsh";
import { Button, Checkbox, Dialog, Input, Menu, MenuItem, Progress, Radio, RadioGroup, Sheet, Switch, Tab, Tabs } from "nsh/base";
import { Glyph } from "./chrome";
import {
  beginSessionProbe,
  loginFocus,
  applyParsed,
  loginOpen,
  noteSlow,
  notify,
  openLogin,
  platformName,
  probe,
  probePlatform,
  profile,
  type Dash,
  readerUrl,
  rememberSession,
  sessionOf,
  briefingOpen,
  settingsOpen,
  opmlOpen,
  manageOpen,
  addOpen,
  revision,
  activeFolder,
  type PlatformId,
  type Folder,
} from "./model";
import { reuseByKey } from "./stable";
import { SubscriptionSync } from "./sync";
import { browseAgent, IOS_SAFARI, META_SESSION_HTML, loginAgent, WEIBO_LOGIN_SCRIPT, weiboResolveScript } from "./web";

const REFRESH = [
  [30, "30 秒"],
  [60, "1 分钟"],
  [120, "2 分钟"],
  [300, "5 分钟"],
  [600, "10 分钟"],
  [1800, "30 分钟"],
] as const;

export function Overlays() {
  return (
    <View>
      <If when={settingsOpen()}>
        <Settings />
      </If>
      <If when={opmlOpen()}>
        <OpmlSheet />
      </If>
      <If when={manageOpen()}>
        <RssManage />
      </If>
      <If when={loginOpen()}>
        <LoginSheet />
      </If>
      <If when={addOpen()}>
        <AddAccount />
      </If>
      <If when={readerUrl() !== ""}>
        <Reader />
      </If>
      <If when={briefingOpen()}>
        <BriefingSheet />
      </If>
    </View>
  );
}

interface RssFeed {
  index: number;
  title: string;
  host: string;
  url: string;
  posts: number;
  empty: boolean;
  fetched: boolean;
  note: string;
  folder?: string;
}

function RssManage() {
  const rows = signal<RssFeed[]>([]);
  const folders = signal<Folder[]>([]);
  const picked = signal<string[]>([]);
  const onlyEmpty = signal(false);
  const draft = signal("");
  const moveOpen = signal(false);
  const kept = new Map<string, RssFeed>();
  let order: RssFeed[] = [];
  let folderKey = "";
  effect(() => {
    revision();
    const parsed = JSON.parse(Core.rssCatalog()) as { folders?: Folder[]; feeds?: RssFeed[] } | RssFeed[];
    const next = Array.isArray(parsed) ? parsed : Array.isArray(parsed.feeds) ? parsed.feeds : [];
    const adopted = reuseByKey(
      kept,
      order,
      next,
      (row) => row.url,
      (row) => `${row.title}|${row.folder ?? ""}|${row.posts}|${row.empty ? 1 : 0}|${row.fetched ? 1 : 0}|${row.note}|${row.host}`,
    );
    order = adopted.items;
    if (adopted.changed) rows.set(adopted.items);
    const nextFolders = Array.isArray(parsed) ? [] : Array.isArray(parsed.folders) ? parsed.folders : [];
    const key = nextFolders.map((folder) => `${folder.id}:${folder.name}`).join("\n");
    if (key !== folderKey) {
      folderKey = key;
      folders.set(nextFolders);
    }
  });
  const folderName = (id: string) => folders().find((folder) => folder.id === id)?.name ?? "";
  const shown = () => {
    const folder = activeFolder();
    const list = rows().filter((row) => {
      if (!folder) return true;
      if (folder === "-") return !row.folder;
      return row.folder === folder;
    });
    return onlyEmpty() ? list.filter((row) => row.empty) : list;
  };
  const emptyCount = () => rows().filter((row) => row.empty).length;
  const selected = () => picked().filter((url) => rows().some((row) => row.url === url));
  const allShown = () => {
    const list = shown();
    return list.length > 0 && list.every((row) => selected().includes(row.url));
  };
  const currentFolder = () => folders().find((folder) => folder.id === activeFolder());
  function toggle(url: string, on: boolean) {
    const current = picked();
    picked.set(on ? (current.includes(url) ? current : [...current, url]) : current.filter((item) => item !== url));
  }
  function selectShown() {
    if (allShown()) {
      const hide = new Set(shown().map((row) => row.url));
      picked.set(picked().filter((url) => !hide.has(url)));
      return;
    }
    const next = new Set(picked());
    for (const row of shown()) next.add(row.url);
    picked.set([...next]);
  }
  function clearEmpty() {
    const started = Date.now();
    const env = applyParsed(JSON.parse(Core.clearEmptyRss()));
    noteSlow("rss-clear-empty", started);
    if (!env.ok) {
      notify(env.error || "没有空订阅");
      return;
    }
    picked.set([]);
    notify("已清理空订阅");
  }
  function removeSelected() {
    const urls = selected();
    if (urls.length === 0) return;
    const started = Date.now();
    const env = applyParsed(JSON.parse(Core.removeRss(JSON.stringify(urls))));
    noteSlow("rss-remove", started);
    if (!env.ok) {
      notify(env.error || "没有删掉订阅");
      return;
    }
    picked.set([]);
    notify(`已删除 ${urls.length} 个订阅`);
  }
  function createFolder() {
    const name = draft().trim();
    if (!name) {
      notify("先写文件夹的名字。");
      return;
    }
    const env = applyParsed(JSON.parse(Core.createRssFolder(name)));
    if (!env.ok) {
      notify(env.error || "没有建成文件夹");
      return;
    }
    draft.set("");
    notify("文件夹已建成");
  }
  function renameFolder() {
    const folder = currentFolder();
    const name = draft().trim();
    if (!folder) return;
    if (!name) {
      notify("先写文件夹的名字。");
      return;
    }
    const env = applyParsed(JSON.parse(Core.renameRssFolder(folder.id, name)));
    if (!env.ok) {
      notify(env.error || "没有改成");
      return;
    }
    draft.set("");
    notify("文件夹已改名");
  }
  function removeFolder() {
    const folder = currentFolder();
    if (!folder) return;
    const env = applyParsed(JSON.parse(Core.deleteRssFolder(folder.id)));
    if (!env.ok) {
      notify(env.error || "没有删掉文件夹");
      return;
    }
    if (activeFolder() === folder.id) activeFolder.set("");
    notify("文件夹已删除，里面的订阅回到未分类");
  }
  function moveSelected(folderId: string) {
    const urls = selected();
    if (urls.length === 0) return;
    moveOpen.set(false);
    const env = applyParsed(JSON.parse(Core.moveRss(JSON.stringify(urls), folderId)));
    if (!env.ok) {
      notify(env.error || "没有移动");
      return;
    }
    picked.set([]);
    notify(folderId ? "已移到文件夹" : "已移到未分类");
  }
  const desktop = platform.idiom === "desktop" && platform.os !== "ios";
  const listHeight = () => Math.max(220, windowSize().height - (desktop ? 360 : 420));
  return (
    <Sheet
      open
      side={desktop ? "right" : "bottom"}
      onOpenChange={(open) => {
        if (!open) manageOpen.set(false);
      }}
      backdropClass="bg-black/50"
      popupClass="bg-card"
      popupStyle={desktop ? { width: 420, maxWidth: "100%", height: "100%" } : { height: "86%" }}
    >
      <View class="min-h-0 flex-1 gap-3 p-4" style={{ minHeight: 0 }}>
        <View class="flex-row items-center gap-2">
          <View class="min-w-0 flex-1">
            <Text class="text-lg font-semibold text-card-foreground">管理订阅</Text>
            <Text class="text-xs text-muted-foreground">
              {rows().length} 个订阅 · {emptyCount()} 个没有文章
            </Text>
          </View>
          <Button onPress={() => manageOpen.set(false)} class="px-2 py-1 active:opacity-70">
            <Text class="text-sm text-muted-foreground">关闭</Text>
          </Button>
        </View>
        <View class="flex-row flex-wrap gap-2">
          <Button
            onPress={() => activeFolder.set("")}
            accessibilityLabel="全部订阅"
            class={activeFolder() === "" ? "rounded-full bg-primary px-3 py-1 active:opacity-80" : "rounded-full bg-secondary px-3 py-1 active:opacity-80"}
          >
            <Text class={activeFolder() === "" ? "text-xs font-semibold text-primary-foreground" : "text-xs text-secondary-foreground"}>全部</Text>
          </Button>
          <Button
            onPress={() => activeFolder.set("-")}
            accessibilityLabel="未分类订阅"
            class={activeFolder() === "-" ? "rounded-full bg-primary px-3 py-1 active:opacity-80" : "rounded-full bg-secondary px-3 py-1 active:opacity-80"}
          >
            <Text class={activeFolder() === "-" ? "text-xs font-semibold text-primary-foreground" : "text-xs text-secondary-foreground"}>未分类</Text>
          </Button>
          {folders().map((folder) => (
            <Button
              onPress={() => activeFolder.set(folder.id)}
              accessibilityLabel={`文件夹 ${folder.name}`}
              class={
                activeFolder() === folder.id
                  ? "rounded-full bg-primary px-3 py-1 active:opacity-80"
                  : "rounded-full bg-secondary px-3 py-1 active:opacity-80"
              }
            >
              <Text
                class={activeFolder() === folder.id ? "text-xs font-semibold text-primary-foreground" : "text-xs text-secondary-foreground"}
                numberOfLines={1}
              >
                {folder.name}
              </Text>
            </Button>
          ))}
        </View>
        <View class="flex-row gap-2">
          <Input
            value={draft()}
            onChange={(text) => draft.set(text)}
            onSubmit={createFolder}
            placeholder="文件夹名字"
            class="min-w-0 flex-1 rounded-[10px] bg-muted px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground"
          />
          <Button
            onPress={createFolder}
            accessibilityLabel="新建文件夹"
            class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
          >
            <Text class="text-sm font-semibold text-secondary-foreground">新建</Text>
          </Button>
          <If when={!!currentFolder()}>
            <Button
              onPress={renameFolder}
              accessibilityLabel="重命名文件夹"
              class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
            >
              <Text class="text-sm font-semibold text-secondary-foreground">重命名</Text>
            </Button>
            <Button
              onPress={removeFolder}
              accessibilityLabel="删除文件夹"
              class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
            >
              <Text class="text-sm font-semibold text-destructive">删除</Text>
            </Button>
          </If>
        </View>
        <View class="flex-row items-center gap-2">
          <Switch
            checked={onlyEmpty()}
            onChange={(on) => onlyEmpty.set(on)}
            accessibilityLabel="只看空订阅"
            class="min-w-0 flex-1 flex-row-reverse items-center justify-between gap-3 rounded-[12px] bg-secondary px-3 py-2"
            trackClass="h-6 w-11 flex-row items-center rounded-full bg-muted p-0.5 checked:bg-primary"
            thumbClass="size-5 rounded-full bg-background shadow-sm checked:translate-x-5"
          >
            <Text class="min-w-0 flex-1 text-sm text-secondary-foreground">只看空订阅</Text>
          </Switch>
          <Button onPress={selectShown} class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80">
            <Text class="text-sm font-medium text-secondary-foreground">{allShown() ? "取消全选" : "全选"}</Text>
          </Button>
          <Menu
            open={moveOpen()}
            onOpenChange={(open) => moveOpen.set(open)}
            placement="bottom"
            align="end"
            offset={4}
            popupClass="min-w-36 rounded-[12px] border border-border bg-popover p-1 shadow-md"
            trigger={
              <View accessibilityLabel="移动所选" class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80">
                <Text class="text-sm font-medium text-secondary-foreground">移动所选</Text>
              </View>
            }
          >
            <MenuItem onSelect={() => moveSelected("")} class="rounded-[8px] px-3 py-2 hover:bg-accent">
              <Text class="text-sm text-popover-foreground">未分类</Text>
            </MenuItem>
            {folders().map((folder) => (
              <MenuItem onSelect={() => moveSelected(folder.id)} class="rounded-[8px] px-3 py-2 hover:bg-accent">
                <Text class="text-sm text-popover-foreground" numberOfLines={1}>
                  {folder.name}
                </Text>
              </MenuItem>
            ))}
          </Menu>
        </View>
        <List
          each={shown()}
          keyExtractor={(item) => item.url}
          estimatedItemSize={64}
          style={{ height: listHeight() }}
          contentClass="gap-1"
          empty={
            <View class="items-center px-4 py-10">
              <Text class="text-sm text-muted-foreground">{onlyEmpty() ? "没有空订阅" : "还没有订阅"}</Text>
            </View>
          }
        >
          {(item) => <RssRow item={item} picked={picked} onToggle={toggle} place={folderName(item().folder ?? "")} />}
        </List>
        <View class="flex-row flex-wrap gap-2">
          <Button
            onPress={clearEmpty}
            disabled={emptyCount() === 0}
            class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80 disabled:opacity-40"
          >
            <Text class="text-sm font-semibold text-secondary-foreground">清理空订阅{emptyCount() > 0 ? ` · ${emptyCount()}` : ""}</Text>
          </Button>
          <Button
            onPress={removeSelected}
            disabled={selected().length === 0}
            class="rounded-[10px] bg-destructive px-3 py-2 active:opacity-80 disabled:opacity-40"
          >
            <Text class="text-sm font-semibold text-white">
              删除所选{selected().length > 0 ? ` · ${selected().length}` : ""}
            </Text>
          </Button>
        </View>
      </View>
    </Sheet>
  );
}

function RssRow(props: {
  item: () => RssFeed;
  picked: () => string[];
  onToggle: (url: string, on: boolean) => void;
  place: string;
}) {
  const detail = () => {
    const feed = props.item();
    if (feed.empty) return feed.fetched ? "没有文章" : "还没拉到内容";
    return `${feed.posts} 篇`;
  };
  return (
    <Checkbox
      checked={props.picked().includes(props.item().url)}
      onChange={(next) => props.onToggle(props.item().url, next)}
      accessibilityLabel={props.item().title}
      class="flex-row items-center gap-3 rounded-[10px] px-1 py-2 hover:bg-accent"
      indicatorClass="h-5 w-5 items-center justify-center rounded-[6px] border border-border checked:border-primary checked:bg-primary"
      indicator={
        <Text class="text-xs font-semibold text-primary-foreground">
          {props.picked().includes(props.item().url) ? "✓" : ""}
        </Text>
      }
    >
      <View class="min-w-0 flex-1">
        <Text class="text-sm font-medium text-foreground" numberOfLines={1}>
          {props.item().title}
        </Text>
        <Text class="text-xs text-muted-foreground" numberOfLines={1}>
          {props.place || "未分类"} · {props.item().host} · {detail()}
        </Text>
      </View>
    </Checkbox>
  );
}

function OpmlSheet() {
  const text = signal("");
  const problem = signal("");
  async function copyOut() {
    const opml = Core.exportOpml();
    await Clipboard.writeText(opml);
    text.set(opml);
    notify("OPML 已复制");
  }
  async function pasteIn() {
    text.set(await Clipboard.readText());
  }
  function importFile() {
    const body = JSON.parse(Core.importOpmlFile()) as Dash & { cancelled?: boolean };
    if (body.cancelled) return;
    if (!body.ok) {
      const message = body.error || "没有导入";
      problem.set(message);
      notify(message);
      return;
    }
    const env = applyParsed(body);
    if (!env.ok) {
      problem.set(env.error || "没有导入");
      notify(env.error || "没有导入");
      return;
    }
    problem.set("");
    notify("订阅已导入");
    opmlOpen.set(false);
  }
  function exportFile() {
    const body = JSON.parse(Core.exportOpmlFile()) as {
      ok?: boolean;
      cancelled?: boolean;
      error?: string;
      path?: string;
    };
    if (body.cancelled) return;
    if (!body.ok) {
      const message = body.error || "没有保存";
      problem.set(message);
      notify(message);
      return;
    }
    problem.set("");
    notify("已保存");
  }
  function importText() {
    const raw = text().trim();
    if (!raw) {
      problem.set("先粘贴一份 OPML。");
      notify("先粘贴一份 OPML。");
      return;
    }
    const env = applyParsed(JSON.parse(Core.importOpml(raw)));
    if (!env.ok) {
      problem.set(env.error || "没有导入");
      notify(env.error || "没有导入");
      return;
    }
    problem.set("");
    notify("订阅已导入");
    opmlOpen.set(false);
  }
  return (
    <Dialog
      open
      animation="fade"
      presentation="fullScreen"
      onOpenChange={(open) => {
        if (!open) opmlOpen.set(false);
      }}
      backdropClass="bg-black/50"
      class="m-auto w-[460px] overflow-hidden rounded-[16px] border border-border bg-card shadow-sm"
      style={{ maxHeight: "80%" }}
    >
      <View class="gap-3 p-5">
        <View class="flex-row items-center">
          <Text class="flex-1 text-lg font-semibold text-card-foreground">OPML</Text>
          <Button onPress={() => opmlOpen.set(false)} class="px-2 py-1 active:opacity-70">
            <Text class="text-sm text-muted-foreground">关闭</Text>
          </Button>
        </View>
        <Text class="text-xs leading-5 text-muted-foreground">
          从磁盘选一份 .opml 或 .xml，或把当前订阅存成 .opml。剪贴板仍可作为备用。
        </Text>
        <View class="flex-row flex-wrap gap-2">
          <Button onPress={importFile} class="rounded-[10px] bg-primary px-3 py-2 active:opacity-80">
            <Text class="text-sm font-semibold text-primary-foreground">选择 OPML 文件</Text>
          </Button>
          <Button onPress={exportFile} class="rounded-[10px] bg-primary px-3 py-2 active:opacity-80">
            <Text class="text-sm font-semibold text-primary-foreground">保存为 OPML 文件</Text>
          </Button>
        </View>
        <View class="flex-row flex-wrap gap-2">
          <Button onPress={() => void copyOut()} class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80">
            <Text class="text-sm font-semibold text-secondary-foreground">复制到剪贴板</Text>
          </Button>
          <Button onPress={() => void pasteIn()} class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80">
            <Text class="text-sm font-semibold text-secondary-foreground">从剪贴板填入</Text>
          </Button>
        </View>
        <Input
          multiline
          value={text()}
          onChange={(value) => text.set(value)}
          placeholder="把 OPML 贴在这里"
          class="text-xs text-foreground placeholder:text-muted-foreground"
          style={{ height: 160, padding: 10 }}
        />
        <If when={!!problem()}>
          <Text class="text-xs text-destructive">{problem()}</Text>
        </If>
        <Button onPress={importText} class="self-start rounded-[10px] bg-secondary px-3 py-2 active:opacity-80">
          <Text class="text-sm font-semibold text-secondary-foreground">导入这些订阅</Text>
        </Button>
      </View>
    </Dialog>
  );
}

/** Keep X popups in this web view. `about:blank` must not wipe the document. */
const STAY_IN_VIEW =
  "window.open = function (url) { var next = url == null ? '' : String(url); if (next.indexOf('https://') === 0 || next.indexOf('http://') === 0) location.href = next; return null; };";

function LoginSheet() {
  const tab = signal<PlatformId>(loginFocus());
  const url = signal(Core.loginPage(loginFocus(), platform.os));
  const shown = signal(true);
  const status = signal("正在打开登录页…");
  const ios = platform.os === "ios";
  function retarget(id: PlatformId) {
    tab.set(id);
    url.set(Core.loginPage(id, platform.os));
    status.set("正在打开登录页…");
    shown.set(false);
    setTimeout(() => shown.set(true), 32);
  }
  return (
    <Dialog
      open
      animation={ios ? "slide" : "fade"}
      presentation="fullScreen"
      onOpenChange={(open) => {
        if (!open) finishLogin(tab(), url());
      }}
      backdropClass="bg-black/50"
      class={
        ios
          ? "flex-1 overflow-hidden bg-background"
          : "m-4 flex-1 overflow-hidden rounded-[16px] border border-border bg-card shadow-sm"
      }
      style={
        ios
          ? {
              paddingTop: "env(safe-area-inset-top)",
              paddingRight: "env(safe-area-inset-right)",
              paddingBottom: "env(safe-area-inset-bottom)",
              paddingLeft: "env(safe-area-inset-left)",
            }
          : undefined
      }
    >
      <View class="flex-row items-center gap-3 border-b border-border px-4 py-3">
        <View class="min-w-0 flex-1">
          <Text class="text-base font-semibold text-card-foreground">登录 {platformName(tab())}</Text>
          <Text class="text-xs text-muted-foreground" numberOfLines={1}>
            {url()}
          </Text>
        </View>
        <Button
          onPress={() => retarget(tab())}
          class={
            ios
              ? "min-h-11 justify-center rounded-[12px] bg-secondary px-4 active:opacity-80"
              : "rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
          }
        >
          <Text class={ios ? "text-[15px] font-semibold text-secondary-foreground" : "text-sm font-semibold text-secondary-foreground"}>
            重新加载
          </Text>
        </Button>
        <Button
          onPress={() => finishLogin(tab(), url())}
          class={
            ios
              ? "min-h-11 justify-center rounded-[12px] bg-primary px-4 active:opacity-80"
              : "rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
          }
        >
          <Text
            class={
              ios ? "text-[15px] font-semibold text-primary-foreground" : "text-sm font-semibold text-secondary-foreground"
            }
          >
            完成
          </Text>
        </Button>
      </View>
      <Tabs class="flex-row gap-2 px-4 pt-3">
        <LoginTab id="x" label="X" tab={tab} onPick={retarget} />
        <LoginTab id="threads" label="Threads" tab={tab} onPick={retarget} />
        <LoginTab id="instagram" label="Instagram" tab={tab} onPick={retarget} />
        <LoginTab id="weibo" label="微博" tab={tab} onPick={retarget} />
      </Tabs>
      <Text class="px-4 py-2 text-xs leading-5 text-muted-foreground">{loginHint(tab(), url())}</Text>
      <Text class="px-4 pb-2 text-xs text-muted-foreground">{status()}</Text>
      <View style={{ flex: 1, minHeight: 360 }}>
        <If when={shown() && ios && (tab() === "instagram" || tab() === "threads")}>
          <MetaSessionLogin platform={tab() === "threads" ? "threads" : "instagram"} url={url} status={status} />
        </If>
        <If when={shown() && !(ios && (tab() === "instagram" || tab() === "threads"))}>
          <View style={{ flex: 1 }}>
            <LoginWeb platform={tab()} src={Core.loginPage(tab(), platform.os)} url={url} status={status} />
          </View>
        </If>
      </View>
    </Dialog>
  );
}

function LoginTab(props: {
  id: PlatformId;
  label: string;
  tab: { (): PlatformId };
  onPick: (id: PlatformId) => void;
}) {
  return (
    <Tab
      selected={props.tab() === props.id}
      onPress={() => props.onPick(props.id)}
      class={
        platform.os === "ios"
          ? "group min-h-11 justify-center rounded-full bg-secondary px-4 selected:bg-primary"
          : "group justify-center rounded-full bg-secondary px-3 py-1.5 selected:bg-primary"
      }
    >
      <Text class="text-sm text-secondary-foreground group-selected:font-semibold group-selected:text-primary-foreground">
        {props.label}
      </Text>
    </Tab>
  );
}

function MetaSessionLogin(props: {
  platform: "instagram" | "threads";
  url: { set(value: string): void };
  status: { set(value: string): void };
}) {
  const user = signal("");
  const pass = signal("");
  const code = signal("");
  const phase = signal<"password" | "code">("password");
  const busy = signal(false);
  const problem = signal("");
  const ready = signal(false);
  let web!: YNode;
  const threads = props.platform === "threads";
  const origin = threads ? "https://www.threads.com" : "https://www.instagram.com";
  const page = threads ? "https://www.threads.com/login" : "https://www.instagram.com/accounts/login/";
  const appId = threads ? "238260118697367" : "936619743392459";
  function submit() {
    if (!ready() || busy()) return;
    if (!user().trim() || (phase() === "password" ? !pass() : !code().trim())) {
      problem.set(phase() === "password" ? "填用户名和密码" : "填验证码");
      return;
    }
    busy.set(true);
    problem.set("");
    props.status.set("正在登录…");
    web.postMessage(
      JSON.stringify({
        kind: phase() === "code" ? "code" : "password",
        username: user().trim(),
        password: pass(),
        code: code().trim(),
        origin,
        page,
        appId,
      }),
    );
  }
  return (
    <View class="min-h-0 flex-1 gap-3 px-4 py-3">
      <Text class="text-xs leading-5 text-muted-foreground">
        {threads ? "Threads" : "Instagram"} 的登录页在 iPhone 的内嵌网页里是白的。在这里登录，会话写进这个应用的网页存储，然后点完成。
      </Text>
      <Input
        value={user()}
        onChange={(value) => user.set(value)}
        placeholder="用户名"
        autoCapitalize="none"
        autoCorrect={false}
        class="min-h-11 rounded-[12px] bg-muted px-3 text-[16px] text-foreground"
      />
      <If when={phase() === "password"}>
        <Input
          value={pass()}
          onChange={(value) => pass.set(value)}
          placeholder="密码"
          masked
          autoCapitalize="none"
          autoCorrect={false}
          class="min-h-11 rounded-[12px] bg-muted px-3 text-[16px] text-foreground"
        />
      </If>
      <If when={phase() === "code"}>
        <Input
          value={code()}
          onChange={(value) => code.set(value)}
          placeholder="验证码"
          autoCapitalize="none"
          autoCorrect={false}
          keyboardType="number-pad"
          class="min-h-11 rounded-[12px] bg-muted px-3 text-[16px] text-foreground"
        />
      </If>
      <If when={!!problem()}>
        <Text class="text-xs text-destructive">{problem()}</Text>
      </If>
      <Button
        onPress={submit}
        disabled={!ready() || busy()}
        class="min-h-11 items-center justify-center rounded-[12px] bg-primary px-4 active:opacity-80 disabled:opacity-40"
      >
        <Text class="text-[15px] font-semibold text-primary-foreground">{busy() ? "正在登录…" : "登录"}</Text>
      </Button>
      <WebView
        ref={web}
        html={META_SESSION_HTML}
        baseUrl={page}
        userAgent={IOS_SAFARI}
        onLoad={() => {
          ready.set(true);
          props.status.set("填写账号后点登录");
          props.url.set(page);
        }}
        onMessage={(event) => {
          let body: { ready?: boolean; ok?: boolean; twoFactor?: boolean; message?: string; checkpoint?: string };
          try {
            body = JSON.parse(event.data);
          } catch {
            return;
          }
          if (body.ready) {
            ready.set(true);
            return;
          }
          busy.set(false);
          if (body.ok) {
            rememberSession(props.platform, "in");
            problem.set("");
            props.status.set("已登录，可以点完成。");
            props.url.set(threads ? "https://www.threads.com/" : "https://www.instagram.com/");
            return;
          }
          if (body.twoFactor) {
            phase.set("code");
            problem.set(body.message || "再输入验证码");
            props.status.set("再输入验证码");
            return;
          }
          const message = (body.message || "没有登录上").replace(/\s+/g, " ").slice(0, 80);
          problem.set(message);
          props.status.set(message);
        }}
        style={{ position: "absolute", left: -4200, top: 0, width: 320, height: 480, opacity: 0 }}
      />
    </View>
  );
}

function LoginWeb(props: {
  platform: PlatformId;
  src: string;
  url: { set(value: string): void };
  status: { set(value: string): void };
}) {
  let opened = false;
  let announced = false;
  let loggedNav = "";
  effect(() => {
    const timer = setTimeout(() => {
      if (!opened) props.status.set("登录页没有显示出来。点重新加载再试一次。");
    }, 12000);
    cleanup(() => clearTimeout(timer));
  });
  return (
    <WebView
      src={props.src}
      userAgent={loginAgent(props.platform)}
      injectedJavaScriptBeforeContentLoaded={props.platform === "weibo" ? WEIBO_LOGIN_SCRIPT : STAY_IN_VIEW}
      style={
        platform.os === "ios"
          ? { flex: 1, width: "100%", height: "100%", minHeight: Math.max(360, windowSize().height - 220) }
          : { flex: 1, width: "100%", height: "100%" }
      }
      desktop={{ allowsBackForwardNavigationGestures: true }}
      ios={{ allowsBackForwardNavigationGestures: true }}
      onLoadStart={() => {
        opened = false;
        props.status.set("正在打开登录页…");
      }}
      onLoad={(nav) => {
        opened = true;
        if (nav.url) props.url.set(nav.url);
        if (!announced) {
          announced = true;
          props.status.set("登录页已打开");
        }
      }}
      onError={(event) => {
        opened = true;
        props.status.set(event.message || "打不开这个登录页。点重新加载再试一次。");
      }}
      onNavigationStateChange={(nav) => {
        if (nav.url) props.url.set(nav.url);
        if (props.platform === "weibo" && nav.url && nav.url !== loggedNav) {
          loggedNav = nav.url;
          Core.weiboLoginLog(`nav ${nav.url}`);
        }
      }}
      onMessage={(event) => onWeiboLoginMessage(props.platform, event.data, props.status)}
    />
  );
}

function onWeiboLoginMessage(
  kind: PlatformId,
  data: string,
  status: { set(value: string): void },
) {
  if (kind !== "weibo") return;
  let body: { type?: string; event?: string; src?: string; tag?: string; login?: boolean; href?: string };
  try {
    body = JSON.parse(data);
  } catch {
    return;
  }
  if (body.type !== "weibo-login") return;
  if (body.event === "fail") Core.weiboLoginLog(`fail ${body.tag || "resource"} ${body.src || ""}`);
  if (body.event === "config") {
    Core.weiboLoginLog(`config login=${body.login ? "true" : "false"} ${body.href || ""}`);
    if (body.login) {
      rememberSession("weibo", "in");
      status.set("微博已登录，可以点完成。");
    }
  }
}

function loginHint(kind: PlatformId, url: string) {
  if (Core.sessionFromProbe(kind, url, false) === "pending") return "已登录，可以点完成。";
  if (kind === "threads") return "在这里登录 Threads，登录后点完成。可以用 Instagram 账号。";
  if (kind === "instagram") return "在这里登录 Instagram，登录后点完成。";
  if (kind === "weibo") {
    return platform.os === "ios"
      ? "在这里登录微博，登录后点完成。"
      : "用微博 App 扫左侧二维码，或用验证码、密码登录。扫完后会回到手机微博并确认登录。";
  }
  return "在这里登录 X，登录后点完成。";
}

function finishLogin(platform: PlatformId, url: string) {
  if (!loginOpen()) return;
  // Weibo `/` and `/u/` are public. The probe's api/config call decides.
  if (platform !== "weibo" && Core.sessionFromProbe(platform, url, false) === "pending") rememberSession(platform, "in");
  beginSessionProbe();
  loginOpen.set(false);
}

function Reader() {
  const url = () => readerUrl();
  const ios = platform.os === "ios";
  return (
    <Dialog
      open
      animation={ios ? "slide" : "fade"}
      presentation="fullScreen"
      onOpenChange={(open) => {
        if (!open) readerUrl.set("");
      }}
      backdropClass="bg-black/50"
      class={
        ios
          ? "flex-1 overflow-hidden bg-background"
          : "m-4 flex-1 overflow-hidden rounded-[16px] border border-border bg-card shadow-sm"
      }
      style={
        ios
          ? {
              paddingTop: "env(safe-area-inset-top)",
              paddingRight: "env(safe-area-inset-right)",
              paddingBottom: "env(safe-area-inset-bottom)",
              paddingLeft: "env(safe-area-inset-left)",
            }
          : undefined
      }
    >
      <View class="flex-row items-center gap-2 border-b border-border px-4 py-3">
        <Text class="min-w-0 flex-1 text-base font-semibold text-card-foreground" numberOfLines={1}>
          原帖
        </Text>
        <Button
          onPress={() => void openUrl(url())}
          class={ios ? "min-h-11 justify-center rounded-[12px] bg-secondary px-4 active:opacity-80" : "rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"}
        >
          <Text class="text-sm font-semibold text-secondary-foreground">在浏览器中打开</Text>
        </Button>
        <Button
          onPress={() => readerUrl.set("")}
          class={ios ? "min-h-11 justify-center rounded-[12px] bg-primary px-4 active:opacity-80" : "rounded-[10px] bg-primary px-3 py-2 active:opacity-80"}
        >
          <Text class="text-sm font-semibold text-primary-foreground">关闭</Text>
        </Button>
      </View>
      <WebView
        src={url()}
        style={platform.os === "ios" ? { flex: 1, minHeight: Math.max(320, windowSize().height - 96) } : { flex: 1 }}
        desktop={{ allowsBackForwardNavigationGestures: true }}
        ios={{ allowsBackForwardNavigationGestures: true }}
      />
    </Dialog>
  );
}

function AddAccount() {
  const source = signal<PlatformId | "">("");
  const draft = signal("");
  const error = signal("");
  const resolving = signal(false);
  const resolveUrl = signal("");
  const resolveQuery = signal("");
  let weiboSaved = false;
  const ios = platform.os === "ios";

  function close() {
    addOpen.set(false);
    source.set("");
    draft.set("");
    error.set("");
    resolving.set(false);
    weiboSaved = false;
  }

  function placeInFolder(id: PlatformId) {
    const want = activeFolder();
    if (!want || want === "-") return;
    const cards = profile()[id].cards;
    const handle = cards[cards.length - 1]?.handle;
    if (handle) applyParsed(JSON.parse(Core.moveCardFolder(id, handle, want)));
  }

  function fail(message: string) {
    error.set(message);
    notify(message);
    resolving.set(false);
    weiboSaved = false;
  }

  function saveWeibo(uid: string, name: string, avatar: string, query: string, via: string) {
    if (weiboSaved) return;
    weiboSaved = true;
    resolving.set(false);
    const at = profile().weibo.cards.length;
    const env = applyParsed(JSON.parse(Core.addWeibo(at, uid, name, avatar, query, via)));
    if (!env.ok) {
      fail(env.error || "没有加上");
      return;
    }
    placeInFolder("weibo");
    close();
    notify("已添加");
  }

  function commit() {
    const id = source();
    if (id !== "x" && id !== "threads" && id !== "instagram" && id !== "weibo" && id !== "rss") return;
    const value = draft().trim();
    if (!value) {
      fail(id === "rss" ? "先贴上订阅地址。" : "先写下账号。");
      return;
    }
    if (id !== "weibo") {
      const at = profile()[id].cards.length;
      const env = applyParsed(JSON.parse(Core.setHandle(id, at, value)));
      if (!env.ok) {
        fail(env.error || "没有加上");
        return;
      }
      placeInFolder(id);
      close();
      notify("已添加");
      return;
    }
    if (resolving()) return;
    let query: { ok?: boolean; needsResolve?: boolean; uid?: string; q?: string; via?: string; name?: string; avatar?: string; error?: string };
    try {
      query = JSON.parse(Core.weiboQuery(value));
    } catch {
      fail("没找到这个微博账号");
      return;
    }
    if (!query.ok) {
      fail(query.error || "没找到这个微博账号");
      return;
    }
    if (query.needsResolve) {
      weiboSaved = false;
      error.set("");
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
      if (resolving() && !weiboSaved) fail("没找到这个微博账号");
    }, 12000);
    cleanup(() => clearTimeout(timer));
  });

  const picked = () => source();
  return (
    <Dialog
      open
      animation={ios ? "slide" : "fade"}
      presentation="fullScreen"
      onOpenChange={(open) => {
        if (!open) close();
      }}
      backdropClass="bg-black/50"
      class={
        ios
          ? "flex-1 overflow-hidden bg-background"
          : "m-auto w-[420px] overflow-hidden rounded-[16px] border border-border bg-card shadow-sm"
      }
      style={ios ? { paddingTop: "env(safe-area-inset-top)", paddingBottom: "env(safe-area-inset-bottom)" } : undefined}
    >
      <View class="gap-3 p-5">
        <View class="flex-row items-center">
          <Text class="flex-1 text-lg font-semibold text-card-foreground">添加账号</Text>
          <Button onPress={close} class="px-2 py-1 active:opacity-70">
            <Text class="text-sm text-muted-foreground">关闭</Text>
          </Button>
        </View>
        <Text class="text-xs leading-5 text-muted-foreground">
          {activeFolder() && activeFolder() !== "-" ? "会加进当前文件夹。" : "先选来源，再写下账号或订阅地址。"}
        </Text>
        <View class="flex-row flex-wrap gap-2">
          <AddSourceChip id="x" label="添加 X" source={picked} onPick={(id) => source.set(id)} />
          <AddSourceChip id="threads" label="添加 Threads" source={picked} onPick={(id) => source.set(id)} />
          <AddSourceChip id="instagram" label="添加 Instagram" source={picked} onPick={(id) => source.set(id)} />
          <AddSourceChip id="weibo" label="添加微博账号" source={picked} onPick={(id) => source.set(id)} />
          <AddSourceChip id="rss" label="添加 RSS" source={picked} onPick={(id) => source.set(id)} />
        </View>
        <If when={!!picked()}>
          <Input
            value={draft()}
            onChange={(text) => draft.set(text)}
            onSubmit={commit}
            placeholder={
              picked() === "weibo"
                ? "昵称、数字 uid，或主页链接"
                : picked() === "rss"
                  ? "订阅地址，例如 https://example.com/feed.xml"
                  : "@handle"
            }
            autoCapitalize="none"
            autoCorrect={false}
            returnKeyType="go"
            class="rounded-[10px] bg-muted px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground"
          />
          <Button onPress={commit} accessibilityLabel="完成" class="self-start rounded-[10px] bg-primary px-3 py-2 active:opacity-80">
            <Text class="text-sm font-semibold text-primary-foreground">完成</Text>
          </Button>
        </If>
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
                else if (!weiboSaved) fail("没找到这个微博账号");
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
                fail("没找到这个微博账号");
                return;
              }
              if (hit.ok && hit.uid) saveWeibo(hit.uid, hit.name || resolveQuery(), hit.avatar || "", resolveQuery(), "search");
              else if (!weiboSaved) fail(hit.error || "没找到这个微博账号");
            }}
            style={{ position: "absolute", left: -4200, top: 0, width: 480, height: 900, opacity: 0 }}
          />
        </If>
        <If when={!!error()}>
          <Text class="text-xs text-destructive">{error()}</Text>
        </If>
      </View>
    </Dialog>
  );
}

function AddSourceChip(props: { id: PlatformId; label: string; source: () => string; onPick: (id: PlatformId) => void }) {
  const on = () => props.source() === props.id;
  return (
    <Button
      onPress={() => props.onPick(props.id)}
      accessibilityLabel={props.label}
      class={on() ? "rounded-full bg-primary px-3 py-1.5 active:opacity-80" : "rounded-full bg-secondary px-3 py-1.5 active:opacity-80"}
    >
      <Text class={on() ? "text-xs font-semibold text-primary-foreground" : "text-xs font-medium text-secondary-foreground"}>{props.label}</Text>
    </Button>
  );
}

function Settings() {
  const ios = platform.os === "ios";
  const keywords = signal(profile().settings.muteKeywords.join("\n"));
  const incoming = signal("");
  const problem = signal("");

  function saveKeywords() {
    const started = Date.now();
    const env = applyParsed(JSON.parse(Core.setMute(keywords())));
    noteSlow("set-mute", started);
    if (env.ok) notify("屏蔽词已保存");
  }

  function chooseRefresh(seconds: number) {
    const started = Date.now();
    applyParsed(JSON.parse(Core.setRefresh(seconds)));
    noteSlow("set-refresh", started);
  }

  function chooseTheme(theme: string) {
    const started = Date.now();
    const env = applyParsed(JSON.parse(Core.setTheme(theme)));
    noteSlow("set-theme", started);
    if (!env.ok) return;
    setColorScheme(theme === "light" || theme === "dark" ? theme : "system");
  }

  async function copyOut() {
    const started = Date.now();
    const json = Core.exportProfile();
    noteSlow("export-profile", started);
    await Clipboard.writeText(json);
    incoming.set(json);
    notify("配置已复制，也可以从下面的文本框拿走");
  }

  async function pasteIn() {
    const text = await Clipboard.readText();
    incoming.set(text);
  }

  function importText() {
    const raw = incoming().trim();
    if (!raw) {
      problem.set("先粘贴一份配置。");
      return;
    }
    let env;
    try {
      env = applyParsed(JSON.parse(Core.normalizeProfile(raw)));
    } catch {
      problem.set("JSON 无法读取。");
      return;
    }
    if (!env.ok) {
      problem.set(env.error);
      return;
    }
    problem.set("");
    const theme = profile().settings.theme;
    setColorScheme(theme === "light" || theme === "dark" ? theme : "system");
    notify("配置已导入");
    settingsOpen.set(false);
  }

  // A Mac dialog is a transparent layer. The popup shrink-wraps. A Scroll
  // with only flex:1 then lays out as the title row, and overflow clips
  // 账号登录 and the rest. An explicit height is the viewport, as on the
  // subscription list.
  const tall = () => {
    const h = Math.max(windowSize().height, 1);
    return Math.max(320, Math.min(h - 32, Math.round(h * 0.88)));
  };
  const body = () => Math.max(240, tall() - (ios ? 64 : 56));
  return (
    <Dialog
      open
      animation={ios ? "slide" : "fade"}
      presentation="fullScreen"
      onOpenChange={(open) => {
        if (!open) settingsOpen.set(false);
      }}
      backdropClass="bg-black/50"
      class={
        ios
          ? "flex-1 overflow-hidden bg-background"
          : "m-auto overflow-hidden rounded-[16px] border border-border bg-card shadow-sm"
      }
      style={
        ios
          ? {
              paddingTop: "env(safe-area-inset-top)",
              paddingRight: "env(safe-area-inset-right)",
              paddingBottom: "env(safe-area-inset-bottom)",
              paddingLeft: "env(safe-area-inset-left)",
            }
          : { width: 460, height: tall() }
      }
    >
      <View class="min-h-0 flex-1" style={{ minHeight: 0, height: ios ? undefined : tall(), flexDirection: "column" }}>
      <View class="flex-row items-center px-5 pt-4">
        <Text class="flex-1 text-lg font-semibold text-card-foreground">设置</Text>
        <Button
          onPress={() => settingsOpen.set(false)}
          class={ios ? "min-h-11 justify-center px-2 active:opacity-70" : "px-2 py-1 active:opacity-70"}
        >
          <Text class={ios ? "text-[15px] text-foreground" : "text-sm text-muted-foreground"}>关闭</Text>
        </Button>
      </View>

      <Scroll style={{ flex: 1, minHeight: 0, height: body() }} contentClass="gap-3 px-5 pb-5 pt-3">
      <Text class="text-xs leading-5 text-muted-foreground">
        脉搏（Pulse）。关注的账号、已读和设置留在这台设备上。第一次打开会接上以前 TheySay 的配置。
      </Text>

      <AccountLogin />

      <Text class="text-xs font-medium text-muted-foreground">刷新间隔</Text>
      <RadioGroup class="flex-row flex-wrap gap-2">
        {REFRESH.map(([seconds, label]) => (
          <Radio
            checked={profile().settings.refreshSecs === seconds}
            onChange={() => chooseRefresh(seconds)}
            class={
              ios
                ? "group min-h-11 justify-center rounded-full bg-secondary px-4 checked:bg-primary"
                : "group justify-center rounded-full bg-secondary px-3 py-1.5 checked:bg-primary"
            }
          >
            <Text class="text-xs text-secondary-foreground group-checked:font-semibold group-checked:text-primary-foreground">
              {label}
            </Text>
          </Radio>
        ))}
      </RadioGroup>
      <Text class="text-xs text-muted-foreground">默认 2 分钟。太密容易碰到 X 的限制。</Text>

      <Text class="text-xs font-medium text-muted-foreground">外观</Text>
      <RadioGroup class="flex-row gap-2">
        <ThemeChoice id="light" label="浅色" onPick={chooseTheme} />
        <ThemeChoice id="dark" label="深色" onPick={chooseTheme} />
        <ThemeChoice id="system" label="跟随系统" onPick={chooseTheme} />
      </RadioGroup>

      <Text class="text-xs font-medium text-muted-foreground">早报</Text>
      <SettingSwitch
        label="本机生成早报"
        checked={profile().settings.briefingAuto !== false}
        onChange={(on) => applyParsed(JSON.parse(Core.setBriefingAuto(on)))}
      />
      <BriefingHour />
      <Button onPress={() => briefingOpen.set(true)} class="self-start rounded-[10px] bg-secondary px-3 py-2 active:opacity-80">
        <Text class="text-sm font-semibold text-secondary-foreground">打开早报</Text>
      </Button>
      <Text class="text-xs leading-5 text-muted-foreground">
        打开「本机生成早报」后，这台设备到点会用昨天的星标写一篇中文口播稿。关掉就只在这台设备上跳过，另一台不受影响。手动「生成早报」仍然可以。时间按本地时区，默认早上 8 点。写稿用翻译里的接口，不另存地址或密钥。昨天没有星标就安静跳过。
      </Text>

      <Text class="text-xs font-medium text-muted-foreground">翻译</Text>
      <TranslateSettings />

      <Text class="text-xs font-medium text-muted-foreground">通知</Text>
      <SettingSwitch
        label="新帖通知"
        checked={profile().settings.notify !== false}
        onChange={(on) => setNotify(on)}
      />
      <View class="gap-2">
        <SettingSwitch label="X" checked={platformNotifyOn("x")} onChange={(on) => setPlatformNotify("x", on)} />
        <SettingSwitch
          label="Threads"
          checked={platformNotifyOn("threads")}
          onChange={(on) => setPlatformNotify("threads", on)}
        />
        <SettingSwitch
          label="Instagram"
          checked={platformNotifyOn("instagram")}
          onChange={(on) => setPlatformNotify("instagram", on)}
        />
        <SettingSwitch label="微博" checked={platformNotifyOn("weibo")} onChange={(on) => setPlatformNotify("weibo", on)} />
        <SettingSwitch label="RSS" checked={platformNotifyOn("rss")} onChange={(on) => setPlatformNotify("rss", on)} />
      </View>
      <Text class="text-xs leading-5 text-muted-foreground">
        第一次看到某个账号的帖子不会响。之后的新帖会发一条系统通知，点它会切到那个平台。Mac 上数字在程序坞，iPhone 上在应用图标。关掉通知仍会记下已经看过的帖子。Mac 在应用开着时自己轮询。iPhone 另外登记了后台刷新，这是尽力而为：系统可以一直不叫醒，旁加载经常没有这个权限。醒来后只拉 RSS，再发同样的本地通知和角标。X、Threads、Instagram、微博仍要应用在前台。应用被划掉之后的推送要以后接 APNs，这个包没有推送服务器。
      </Text>
      <Button
        onPress={() => {
          Core.testNotify();
          notify("测试通知已发出");
        }}
        class="self-start rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
      >
        <Text class="text-sm font-semibold text-secondary-foreground">发送测试通知</Text>
      </Button>

      <Text class="text-xs font-medium text-muted-foreground">屏蔽词</Text>
      <Input
        multiline
        value={keywords()}
        onChange={(text) => keywords.set(text)}
        placeholder={"每行一个词或短语\n例如：赠送"}
        class="rounded-xl border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground"
        style={{ height: 88 }}
      />
      <Button onPress={saveKeywords} class="self-start rounded-[10px] bg-secondary px-3 py-2 active:opacity-80">
        <Text class="text-sm font-semibold text-secondary-foreground">保存屏蔽词</Text>
      </Button>
      <Text class="text-xs leading-5 text-muted-foreground">含这些词的帖子不会出现，也不会计入未读。大小写不敏感，多个空格会折成一个。</Text>

      <SubscriptionSync />

      <Text class="text-xs font-medium text-muted-foreground">导入 / 导出</Text>
      <Text class="text-xs leading-5 text-muted-foreground">
        X、Threads、Instagram、微博各自的账号、正在看的平台、刷新、外观和已经读到的帖子都在一份 JSON 里。以前带版面大小或空格子的配置也能导入。这一版的 NSH 没有系统文件框，所以复制到剪贴板，或从下面粘贴进来。存成文件时用 pulse-profile.json。
      </Text>
      <View class="flex-row gap-2">
        <Button onPress={() => void copyOut()} class="rounded-[10px] bg-primary px-3 py-2 active:opacity-80">
          <Text class="text-sm font-semibold text-primary-foreground">复制配置</Text>
        </Button>
        <Button onPress={() => void pasteIn()} class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80">
          <Text class="text-sm font-semibold text-secondary-foreground">从剪贴板填入</Text>
        </Button>
      </View>
      <Input
        multiline
        value={incoming()}
        onChange={(text) => incoming.set(text)}
        placeholder="把导出的 JSON 贴在这里"
        class="text-xs text-foreground placeholder:text-muted-foreground"
        style={{ height: 96, padding: 10 }}
      />
      <If when={!!problem()}>
        <Text class="text-xs text-destructive">{problem()}</Text>
      </If>
      <Button onPress={importText} class="self-start rounded-[10px] bg-secondary px-3 py-2 active:opacity-80">
        <Text class="text-sm font-semibold text-secondary-foreground">导入这段 JSON</Text>
      </Button>
      </Scroll>
      </View>
    </Dialog>
  );
}

function AccountLogin() {
  return (
    <View class="gap-2">
      <Text class="text-xs font-medium text-muted-foreground">账号登录</Text>
      <Text class="text-xs leading-5 text-muted-foreground">
        X、Threads、Instagram 和微博的登录记在这台设备上。RSS 只有订阅，没有账号。
      </Text>
      <LoginRow id="x" />
      <LoginRow id="threads" />
      <LoginRow id="instagram" />
      <LoginRow id="weibo" />
      <RssAccountRow />
    </View>
  );
}

function sessionLine(id: PlatformId) {
  if (probe() !== 0 && probePlatform() === id) return "正在确认";
  const state = sessionOf(id);
  if (state === "in") return "已登录";
  if (state === "blocked") return "被拦住";
  if (state === "out") return "未登录";
  return "还没确认";
}

function LoginRow(props: { id: PlatformId }) {
  const signedIn = () => sessionOf(props.id) === "in";
  const name = () => platformName(props.id);
  return (
    <View class="flex-row items-center gap-3 rounded-[12px] bg-secondary px-3 py-2">
      <Glyph platform={props.id} size={28} />
      <View class="min-w-0 flex-1">
        <Text class="text-sm font-medium text-secondary-foreground" numberOfLines={1}>
          {name()}
        </Text>
        <Text class="text-xs text-muted-foreground" numberOfLines={1}>
          {sessionLine(props.id)}
        </Text>
      </View>
      <Button
        onPress={() => openLogin(props.id)}
        accessibilityLabel={signedIn() ? `重新登录 ${name()}` : `登录 ${name()}`}
        class={
          platform.os === "ios"
            ? "min-h-11 justify-center rounded-[10px] bg-background px-3 active:opacity-80"
            : "rounded-[10px] bg-background px-3 py-1.5 active:opacity-80"
        }
      >
        <Text class="text-xs font-semibold text-foreground">{signedIn() ? "重新登录" : "登录"}</Text>
      </Button>
    </View>
  );
}

function RssAccountRow() {
  return (
    <View class="gap-2 rounded-[12px] bg-secondary px-3 py-2">
      <View class="flex-row items-center gap-3">
        <Glyph platform="rss" size={28} />
        <View class="min-w-0 flex-1">
          <Text class="text-sm font-medium text-secondary-foreground">RSS</Text>
          <Text class="text-xs text-muted-foreground" numberOfLines={1}>
            订阅，不用登录
          </Text>
        </View>
      </View>
      <View class="flex-row gap-2">
        <Button
          onPress={() => manageOpen.set(true)}
          accessibilityLabel="管理订阅"
          class={
            platform.os === "ios"
              ? "min-h-11 flex-1 items-center justify-center rounded-[10px] bg-background active:opacity-80"
              : "rounded-[10px] bg-background px-3 py-1.5 active:opacity-80"
          }
        >
          <Text class="text-xs font-semibold text-foreground">管理订阅</Text>
        </Button>
        <Button
          onPress={() => opmlOpen.set(true)}
          accessibilityLabel="OPML 导入导出"
          class={
            platform.os === "ios"
              ? "min-h-11 flex-1 items-center justify-center rounded-[10px] bg-background active:opacity-80"
              : "rounded-[10px] bg-background px-3 py-1.5 active:opacity-80"
          }
        >
          <Text class="text-xs font-semibold text-foreground">OPML</Text>
        </Button>
      </View>
    </View>
  );
}

function setNotify(on: boolean) {
  const started = Date.now();
  applyParsed(JSON.parse(Core.setNotify(on)));
  noteSlow("set-notify", started);
}

function setPlatformNotify(id: PlatformId, on: boolean) {
  const started = Date.now();
  applyParsed(JSON.parse(Core.setNotifyPlatform(id, on)));
  noteSlow("set-notify-platform", started);
}

function platformNotifyOn(id: PlatformId) {
  const settings = profile().settings;
  if (id === "threads") return settings.notifyThreads !== false;
  if (id === "instagram") return settings.notifyInstagram !== false;
  if (id === "weibo") return settings.notifyWeibo !== false;
  if (id === "rss") return settings.notifyRss !== false;
  return settings.notifyX !== false;
}

function SettingSwitch(props: { label: string; checked: boolean; onChange: (on: boolean) => void }) {
  return (
    <Switch
      checked={props.checked}
      onChange={(next) => props.onChange(next)}
      accessibilityLabel={props.label}
      class={
        platform.os === "ios"
          ? "min-h-11 flex-row-reverse items-center justify-between gap-3 rounded-[12px] bg-secondary px-3"
          : "flex-row-reverse items-center justify-between gap-3 rounded-[12px] bg-secondary px-3 py-2"
      }
      trackClass="h-6 w-11 flex-row items-center rounded-full bg-muted p-0.5 checked:bg-primary"
      thumbClass="size-5 rounded-full bg-background shadow-sm checked:translate-x-5"
    >
      <Text class="min-w-0 flex-1 text-sm text-secondary-foreground">{props.label}</Text>
    </Switch>
  );
}

function BriefingHour() {
  const hour = () => profile().settings.briefingHour ?? 8;
  function pick(next: number) {
    applyParsed(JSON.parse(Core.setBriefingHour(next)));
  }
  return (
    <RadioGroup class="flex-row gap-2">
      {[7, 8, 9].map((item) => (
        <Radio
          checked={hour() === item}
          onChange={() => pick(item)}
          class="group justify-center rounded-full bg-secondary px-3 py-1.5 checked:bg-primary"
        >
          <Text class="text-xs text-secondary-foreground group-checked:font-semibold group-checked:text-primary-foreground">
            {item} 点
          </Text>
        </Radio>
      ))}
    </RadioGroup>
  );
}

function localDay(date: Date) {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

function yesterdayRange(now: Date) {
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  const end = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  return { from: start.getTime(), until: end.getTime(), today: localDay(now) };
}

function BriefingSheet() {
  const script = signal("");
  const note = signal("");
  const busy = signal(false);
  effect(() => {
    if (!briefingOpen()) return;
    try {
      const hit = JSON.parse(Core.briefing()) as { script?: string; note?: string };
      script.set(hit.script || "");
      note.set(hit.note || "");
    } catch {
      note.set("");
    }
  });
  function generate() {
    if (busy()) return;
    busy.set(true);
    note.set("");
    const range = yesterdayRange(new Date());
    void Promise.resolve(Core.makeBriefing(range.from, range.until, range.today, true)).then((raw) => {
      let hit: { ok?: boolean; script?: string; note?: string; empty?: boolean } = {};
      try {
        hit = JSON.parse(String(raw ?? ""));
      } catch {
        hit = { ok: false, note: "早报没有写成。" };
      }
      script.set(hit.script || "");
      note.set(hit.note || (hit.empty ? "昨天没有星标，今天不写早报。" : ""));
      busy.set(false);
    });
  }
  function play() {
    const text = script();
    if (!text) return;
    const hit = JSON.parse(Core.speak(text)) as { ok?: boolean; error?: string };
    if (!hit.ok && hit.error) note.set(hit.error);
  }
  const ios = platform.os === "ios";
  return (
    <Dialog
      open
      animation={ios ? "slide" : "fade"}
      presentation="fullScreen"
      onOpenChange={(open) => {
        if (!open) {
          Core.stopSpeak();
          briefingOpen.set(false);
        }
      }}
      backdropClass="bg-black/50"
      class={
        ios
          ? "flex-1 overflow-hidden bg-background"
          : "m-auto max-h-[80%] w-[440px] overflow-hidden rounded-[16px] border border-border bg-card shadow-sm"
      }
      style={ios ? { paddingTop: "env(safe-area-inset-top)", paddingBottom: "env(safe-area-inset-bottom)" } : undefined}
    >
      <View class="gap-3 p-5">
        <View class="flex-row items-center">
          <Text class="flex-1 text-lg font-semibold text-card-foreground">早报</Text>
          <Button
            onPress={() => {
              Core.stopSpeak();
              briefingOpen.set(false);
            }}
            class="px-2 py-1 active:opacity-70"
          >
            <Text class="text-sm text-muted-foreground">关闭</Text>
          </Button>
        </View>
        <If when={busy()}>
          <Text class="text-xs text-muted-foreground">正在写早报…</Text>
        </If>
        <If when={!!script()}>
          <Scroll style={{ maxHeight: 280 }}>
            <Text selectable class="text-sm leading-6 text-card-foreground">
              {script()}
            </Text>
          </Scroll>
        </If>
        <If when={!script() && !busy()}>
          <Text class="text-sm leading-6 text-muted-foreground">{note() || "还没有今天的早报。"}</Text>
        </If>
        <If when={!!script() && !!note()}>
          <Text class="text-xs leading-5 text-muted-foreground">{note()}</Text>
        </If>
        <View class="flex-row flex-wrap gap-2">
          <Button onPress={generate} class="rounded-[10px] bg-primary px-3 py-2 active:opacity-80">
            <Text class="text-sm font-semibold text-primary-foreground">生成早报</Text>
          </Button>
          <If when={!!script()}>
            <Button onPress={play} class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80">
              <Text class="text-sm font-semibold text-secondary-foreground">播放</Text>
            </Button>
            <Button onPress={() => Core.stopSpeak()} class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80">
              <Text class="text-sm font-semibold text-secondary-foreground">停止</Text>
            </Button>
          </If>
        </View>
      </View>
    </Dialog>
  );
}

function TranslateSettings() {
  const url = signal(profile().settings.translateUrl || "");
  const token = signal(profile().settings.translateToken || "");
  const model = signal(profile().settings.translateModel || "gpt-4o-mini");
  const lang = () => profile().settings.translateLang || "zh";
  function save(next: string) {
    const env = applyParsed(JSON.parse(Core.setTranslate(next, url(), token(), model())));
    if (!env.ok) notify(env.error || "没有保存");
    else notify("翻译设置已保存");
  }
  const choice = (id: string, label: string) => (
    <Radio
      checked={lang() === id}
      onChange={() => save(id)}
      class="group justify-center rounded-full bg-secondary px-3 py-1.5 checked:bg-primary"
    >
      <Text class="text-xs text-secondary-foreground group-checked:font-semibold group-checked:text-primary-foreground">
        {label}
      </Text>
    </Radio>
  );
  return (
    <View class="gap-2">
      <RadioGroup class="flex-row flex-wrap gap-2">
        {choice("zh", "简体中文")}
        {choice("en", "English")}
        {choice("ja", "日本語")}
        {choice("system", "跟随系统")}
      </RadioGroup>
      <Input
        value={url()}
        onChange={(text) => url.set(text)}
        autoCapitalize="none"
        autoCorrect={false}
        placeholder="https://api.openai.com/v1"
        class="rounded-[10px] bg-muted px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground"
      />
      <Input
        masked
        value={token()}
        onChange={(text) => token.set(text)}
        autoCapitalize="none"
        autoCorrect={false}
        placeholder="API Key"
        class="rounded-[10px] bg-muted px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground"
      />
      <Input
        value={model()}
        onChange={(text) => model.set(text)}
        autoCapitalize="none"
        autoCorrect={false}
        placeholder="gpt-4o-mini"
        class="rounded-[10px] bg-muted px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground"
      />
      <Button onPress={() => save(lang())} class="self-start rounded-[10px] bg-secondary px-3 py-2 active:opacity-80">
        <Text class="text-sm font-semibold text-secondary-foreground">保存翻译设置</Text>
      </Button>
      <Text class="text-xs leading-5 text-muted-foreground">
        帖子上的「翻译」把标题和正文发给上面的接口，换成你选的语言。地址是 OpenAI 兼容的根，例如 https://api.openai.com/v1，请求发到 /chat/completions。模型空着就用 gpt-4o-mini。API Key 只存在这台设备上，应用里不写死任何密钥。译文按帖子记下，再次打开不会重发。Apple 的本机翻译要 iOS 18 或 macOS 15，而且是 Swift 接口，这个包接不进去。
      </Text>
    </View>
  );
}

function ThemeChoice(props: { id: string; label: string; onPick: (theme: string) => void }) {
  return (
    <Radio
      checked={profile().settings.theme === props.id}
      onChange={() => props.onPick(props.id)}
      class={
        platform.os === "ios"
          ? "group min-h-11 flex-1 items-center justify-center rounded-xl bg-secondary checked:bg-primary"
          : "group flex-1 items-center rounded-xl bg-secondary py-2 checked:bg-primary"
      }
    >
      <Text class="text-sm text-secondary-foreground group-checked:font-semibold group-checked:text-primary-foreground">
        {props.label}
      </Text>
    </Radio>
  );
}
