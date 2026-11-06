import { Core } from "theysay/native";
import { Scroll, Text, View, effect, platform, windowSize, type YNode } from "nsh";
import { TabBar } from "nsh/mobile";
import { Button, Tab, Tabs } from "nsh/base";
import { Card } from "./card";
import { Timeline } from "./timeline";
import { IconButton } from "./chrome";
import {
  homeBoard,
  applyParsed,
  focusedFreshMs,
  focusedUnread,
  probe,
  profile,
  activeFolder,
  briefingOpen,
  starView,
  scrollHandle,
  noteSlow,
  settingsOpen,
  opmlOpen,
  manageOpen,
  addOpen,
  viewSlots,
  visibleSpan,
} from "./model";
import { BOARD_GAP, BOARD_PAD, boardSpan } from "./board-span";
import { slots } from "./scrapers";
import { freshLabel } from "./text";

export function Shell() {
  const column = () => Core.shellMode(platform.os, platform.idiom, windowSize().width) === "column";
  const ios = () => platform.os === "ios" && column();
  effect(() => {
    const id = activeFolder();
    if (!id || id === "-") return;
    if (!profile().folders.some((folder) => folder.id === id)) activeFolder.set("");
  });
  effect(() => {
    if (profile().focus === "all") return;
    applyParsed(JSON.parse(Core.setFocus("all")));
  });
  return (
    <View class="flex-1 bg-background">
      <If when={ios()}>
        <IosShell />
      </If>
      <If when={column() && !ios()}>
        <PhoneShell />
      </If>
      <If when={!column()}>
        <DesktopShell />
      </If>
    </View>
  );
}

function DesktopShell() {
  return (
    <View class="min-h-0 flex-1 flex-row">
      <Sidebar />
      <View class="min-h-0 min-w-0 flex-1" style={{ flexDirection: "column", minHeight: 0 }}>
        <Toolbar />
        <Home columns={2} />
      </View>
    </View>
  );
}

function Home(props: { columns: number }) {
  return (
    <View class="min-h-0 min-w-0 flex-1" style={{ minHeight: 0 }}>
      <If when={!homeBoard()}>
        <Timeline columns={props.columns} />
      </If>
      <If when={homeBoard()}>
        <CardList columns={props.columns} />
      </If>
    </View>
  );
}

function IosShell() {
  const tab = () => {
    if (settingsOpen()) return 2;
    return homeBoard() ? 1 : 0;
  };
  const unread = () => focusedUnread();
  return (
    <View class="min-h-0 flex-1 bg-background" style={{ minHeight: 0, flexDirection: "column" }}>
      <View
        class="gap-3 border-b border-border bg-background px-4 pb-3"
        style={{ paddingTop: "calc(env(safe-area-inset-top) + 8px)" }}
      >
        <View class="flex-row items-center gap-3">
          <View class="min-w-0 flex-1">
            <View class="flex-row items-center gap-1.5">
              <Text class="text-[11px] font-medium text-muted-foreground">脉搏</Text>
              <Text class="text-[11px] font-medium text-muted-foreground">Pulse</Text>
            </View>
            <Text class="text-[22px] font-semibold tracking-tight text-foreground" numberOfLines={1}>
              {homeBoard() ? "看板" : "动态"}
            </Text>
            <Text class="text-[13px] text-muted-foreground" numberOfLines={1}>
              {iosStatus()}
            </Text>
          </View>
          <IosHit label="添加账号" glyph="＋" onPress={() => addOpen.set(true)} />
          <IosHit label="立即刷新" glyph={slots().length > 0 ? "…" : "↻"} onPress={refreshNow} />
          <If when={unread() > 0}>
            <IosHit label={`全部已读 · ${unread()}`} glyph="✓" onPress={markAll} />
          </If>
        </View>
        <PhoneFolders />
      </View>
      <Home columns={1} />
      <TabBar
        items={[
          { title: "动态", icon: "list.bullet" },
          { title: "看板", icon: "square.grid.2x2" },
          { title: "设置", icon: "gearshape" },
        ]}
        selectedIndex={tab()}
        onChange={(index) => pickIosTab(index)}
      />
    </View>
  );
}

function pickIosTab(index: number) {
  if (index === 2) {
    settingsOpen.set(true);
    return;
  }
  settingsOpen.set(false);
  chooseHome(index === 1 ? "board" : "timeline");
}

function iosStatus() {
  if (probe() !== 0) return "正在确认登录…";
  const focus = profile().focus;
  if (focus === "all") return "按时间排在一起";
  if (focus === "rss") return "订阅按自己的列表刷新";
  return "新帖会记在这里";
}

function IosHit(props: { label: string; glyph: string; onPress: () => void }) {
  return (
    <Button
      onPress={props.onPress}
      accessibilityLabel={props.label}
      class="h-11 w-11 items-center justify-center rounded-[14px] bg-secondary active:opacity-70"
    >
      <Text class="text-lg text-foreground">{props.glyph}</Text>
    </Button>
  );
}

function PhoneShell() {
  return (
    <View class="min-h-0 flex-1" style={{ minHeight: 0, flexDirection: "column" }}>
      <View class="gap-3 border-b border-border px-4 py-3">
        <View class="flex-row items-center gap-3">
          <Mark />
          <View class="min-w-0 flex-1">
            <Text class="text-[17px] font-semibold text-foreground">脉搏</Text>
            <Text class="text-xs text-muted-foreground">Pulse · {homeBoard() ? "看板" : "动态"}</Text>
          </View>
          <Button
            onPress={() => chooseHome(homeBoard() ? "timeline" : "board")}
            accessibilityLabel={homeBoard() ? "打开动态" : "打开看板"}
            class="rounded-[10px] bg-secondary px-3 py-1.5 active:opacity-80"
          >
            <Text class="text-xs font-medium text-secondary-foreground">{homeBoard() ? "动态" : "看板"}</Text>
          </Button>
          <IconButton label="添加账号" glyph="＋" onPress={() => addOpen.set(true)} />
          <IconButton label="设置" glyph="⚙" onPress={() => settingsOpen.set(true)} />
        </View>
        <PhoneFolders />
      </View>
      <Home columns={1} />
    </View>
  );
}

function CardList(props: { columns: number }) {
  const cardHeight = () => Math.max(280, Math.round(windowSize().height / 2));
  const count = () => viewSlots().length + 1;
  let rowOrder: number[] = [];
  const rows = () => {
    const n = Math.ceil(count() / props.columns);
    if (rowOrder.length === n) return rowOrder;
    const out: number[] = [];
    for (let i = 0; i < n; i++) out.push(i);
    rowOrder = out;
    return out;
  };
  let scroller!: YNode;
  effect(() => {
    const height = windowSize().height;
    const cardH = Math.max(280, Math.round(height / 2));
    const next = boardSpan(0, height, cardH, props.columns, Math.max(count(), 1));
    const span = visibleSpan();
    if (span.start === 0 && span.end < next.end) visibleSpan.set(next);
  });
  effect(() => {
    const handle = scrollHandle();
    if (!handle || !scroller) return;
    const index = viewSlots().findIndex((slot) => {
      const id =
        slot.platform === "threads" || slot.platform === "instagram" || slot.platform === "weibo" || slot.platform === "rss"
          ? slot.platform
          : "x";
      return profile()[id]?.cards[slot.index]?.handle === handle;
    });
    if (index < 0) return;
    scroller.scrollTo({ y: Math.floor(index / props.columns) * cardHeight(), animated: true });
    scrollHandle.set("");
  });
  return (
    <Scroll
      ref={scroller}
      zoomable={false}
      scrollable
      style={{ flex: 1, minHeight: 0 }}
      onScroll={(event) => {
        const next = boardSpan(event.offset.y, event.size.height, cardHeight(), props.columns, count());
        const span = visibleSpan();
        if (span.start !== next.start || span.end !== next.end) visibleSpan.set(next);
      }}
    >
      <View style={{ paddingTop: BOARD_PAD, paddingLeft: BOARD_PAD, paddingRight: BOARD_PAD, paddingBottom: 4 }}>
        <If when={count() === 0}>
          <View class="items-center px-6 py-16">
            <Text class="text-sm font-medium text-foreground">还没有账号</Text>
            <Text class="mt-1 text-center text-xs text-muted-foreground">在 X、Threads、Instagram 或微博里添加。</Text>
          </View>
        </If>
        <Each of={rows()} key={(row) => String(row)}>
          {(row) => (
            <View class="flex-row gap-3" style={{ height: cardHeight(), marginBottom: BOARD_GAP }}>
              <Each of={rowCells(row(), props.columns, count())} key={(cell) => String(cell)}>
                {(cell) => <CardSlot cell={cell()} />}
              </Each>
              <If when={rowCells(row(), props.columns, count()).length < props.columns}>
                <View class="min-w-0 flex-1" />
              </If>
            </View>
          )}
        </Each>
      </View>
    </Scroll>
  );
}

/** Mounts a card only while its row is on screen. Off-screen rows keep their height. */
function CardSlot(props: { cell: number }) {
  return (
    <View class="min-w-0 flex-1" style={{ minHeight: 0 }}>
      <If when={props.cell >= visibleSpan().start && props.cell < visibleSpan().end}>
        <Card cell={props.cell} />
      </If>
    </View>
  );
}

function rowCells(row: number, columns: number, count: number) {
  const cells: number[] = [];
  for (let i = 0; i < columns; i++) {
    const cell = row * columns + i;
    if (cell < count) cells.push(cell);
  }
  return cells;
}

function Toolbar() {
  const unread = () => focusedUnread();
  const updated = () => {
    const latest = focusedFreshMs();
    return latest ? freshLabel(latest) : "尚未更新";
  };
  const busy = () => slots().length > 0;
  return (
    <View class="flex-row items-center gap-3 border-b border-border px-4 py-3">
      <View class="min-w-0 flex-1">
        <Text class="text-[17px] font-semibold text-foreground">{homeBoard() ? "看板" : "动态"}</Text>
        <Text class="text-xs text-muted-foreground">
          {folderTitle()} · {updated()}
        </Text>
      </View>
      <Button
        onPress={() => addOpen.set(true)}
        accessibilityLabel="添加账号"
        class="rounded-[10px] bg-primary px-3 py-1.5 active:opacity-80"
      >
        <Text class="text-sm font-semibold text-primary-foreground">添加账号</Text>
      </Button>
      <Button
        onPress={() => manageOpen.set(true)}
        accessibilityLabel="管理订阅"
        class="rounded-[10px] bg-secondary px-3 py-1.5 active:opacity-80"
      >
        <Text class="text-sm font-medium text-secondary-foreground">管理订阅</Text>
      </Button>
      <Button
        onPress={() => chooseHome(homeBoard() ? "timeline" : "board")}
        accessibilityLabel={homeBoard() ? "打开动态" : "打开看板"}
        class="rounded-[10px] bg-secondary px-3 py-1.5 active:opacity-80"
      >
        <Text class="text-sm font-medium text-secondary-foreground">{homeBoard() ? "动态" : "看板"}</Text>
      </Button>
      <IconButton label="立即刷新" glyph="↻" busy={busy()} onPress={refreshNow} />
      <If when={unread() > 0}>
        <IconButton label={`全部已读 · ${unread()}`} glyph="✓" onPress={markAll} />
      </If>
    </View>
  );
}

function chooseHome(home: "timeline" | "board") {
  const started = Date.now();
  applyParsed(JSON.parse(Core.setHome(home)));
  noteSlow("set-home", started);
}

function refreshNow() {
  const started = Date.now();
  applyParsed(JSON.parse(Core.requestRefresh()));
  noteSlow("refresh", started);
}

function markAll() {
  const started = Date.now();
  applyParsed(JSON.parse(Core.markAllRead()));
  noteSlow("mark-all", started);
}

function Sidebar() {
  return (
    <View class="border-r border-sidebar-border bg-sidebar px-3 py-4" style={{ width: 240 }}>
      <View class="flex-row items-center gap-3 px-1">
        <Mark />
        <View class="min-w-0 flex-1">
          <Text class="text-[17px] font-semibold tracking-tight text-sidebar-foreground">脉搏</Text>
          <Text class="text-xs text-muted-foreground">Pulse · 各看各的账号</Text>
        </View>
      </View>

      <Tabs class="mt-8 min-h-0 flex-1 gap-1">
        <Text class="mb-1 px-1 text-xs font-medium text-muted-foreground">文件夹</Text>
        <FolderTab id="" label="全部" hint="全部订阅" mark="全" />
        <FolderTab id="-" label="未分类" hint="还没有文件夹" mark="未" />
        {profile().folders.map((folder) => (
          <FolderTab id={folder.id} label={folder.name} hint="文件夹" mark={folder.name.slice(0, 1) || "夹"} />
        ))}
        <Button
          onPress={showStars}
          accessibilityLabel="星标"
          class="mt-2 rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
        >
          <Text class="text-sm font-medium text-secondary-foreground">星标</Text>
        </Button>
        <Button
          onPress={() => briefingOpen.set(true)}
          accessibilityLabel="早报"
          class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
        >
          <Text class="text-sm font-medium text-secondary-foreground">早报</Text>
        </Button>
        <Button
          onPress={() => addOpen.set(true)}
          accessibilityLabel="添加账号"
          class="mt-2 rounded-[10px] bg-primary px-3 py-2 active:opacity-80"
        >
          <Text class="text-sm font-semibold text-primary-foreground">添加账号</Text>
        </Button>
        <Button
          onPress={() => manageOpen.set(true)}
          accessibilityLabel="管理订阅"
          class="mt-2 rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
        >
          <Text class="text-sm font-medium text-secondary-foreground">管理订阅</Text>
        </Button>
        <Button
          onPress={() => opmlOpen.set(true)}
          accessibilityLabel="OPML 导入导出"
          class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
        >
          <Text class="text-sm font-medium text-secondary-foreground">OPML 导入 / 导出</Text>
        </Button>
        <Text class="mt-2 px-1 text-xs leading-5 text-muted-foreground">
          {probe() !== 0 ? "正在确认登录…" : "点一个文件夹看里面的订阅。登录在设置里。"}
        </Text>
        <View class="flex-1" />
        <Tab
          selected={settingsOpen()}
          onPress={() => settingsOpen.set(true)}
          accessibilityLabel="打开设置"
          class="rounded-[10px] px-3 py-2 hover:bg-sidebar-accent active:opacity-80 selected:bg-accent"
        >
          <Text class="text-sm font-medium text-sidebar-foreground">设置</Text>
        </Tab>
      </Tabs>
      <Text class="mt-2 px-1 text-xs text-muted-foreground">同时最多 {Core.maxScrapers()} 个页面在读</Text>
    </View>
  );
}

function showStars() {
  settingsOpen.set(false);
  starView.set(true);
  visibleSpan.set({ start: 0, end: 6 });
  if (profile().settings.home === "board") {
    applyParsed(JSON.parse(Core.setHome("timeline")));
  }
}

function folderTitle() {
  if (starView()) return "星标";
  const id = activeFolder();
  if (!id) return "全部";
  if (id === "-") return "未分类";
  return profile().folders.find((folder) => folder.id === id)?.name ?? "全部";
}

function folderUnread(id: string) {
  const boards = [profile().x, profile().threads, profile().instagram, profile().weibo, profile().rss];
  let count = 0;
  for (const board of boards) {
    for (const card of board.cards) {
      const inside = !id ? true : id === "-" ? !card.folder : card.folder === id;
      if (inside) count += card.unread ?? 0;
    }
  }
  return count;
}

function chooseFolder(id: string) {
  settingsOpen.set(false);
  starView.set(false);
  activeFolder.set(id);
  visibleSpan.set({ start: 0, end: 6 });
  if (profile().focus !== "all") {
    const started = Date.now();
    applyParsed(JSON.parse(Core.setFocus("all")));
    noteSlow("set-focus", started);
  }
}

function FolderTab(props: { id: string; label: string; hint: string; mark: string }) {
  const count = () => folderUnread(props.id);
  return (
    <Tab
      selected={activeFolder() === props.id && !settingsOpen() && !starView()}
      onPress={() => chooseFolder(props.id)}
      accessibilityLabel={props.id === "" ? "查看全部" : props.id === "-" ? "查看未分类" : `查看文件夹 ${props.label}`}
      class="group w-full flex-row items-center gap-2 rounded-[12px] px-2 py-2 hover:bg-sidebar-accent active:opacity-80 selected:bg-accent"
    >
      <View class="w-1 self-stretch rounded-full group-selected:bg-primary" />
      <View class="h-8 w-8 items-center justify-center rounded-[10px] bg-muted">
        <Text class="text-xs font-semibold text-foreground">{props.mark}</Text>
      </View>
      <View style={{ flexGrow: 1, flexShrink: 1, flexBasis: 0, minWidth: 0 }}>
        <Text class="text-sm font-medium text-sidebar-foreground" numberOfLines={1} ellipsizeMode="tail">
          {props.label}
        </Text>
        <Text class="text-xs text-muted-foreground" numberOfLines={1} ellipsizeMode="tail">
          {props.hint}
        </Text>
      </View>
      <If when={count() > 0}>
        <View class="min-w-5 items-center rounded-full bg-primary px-1.5 py-0.5" style={{ flexGrow: 0, flexShrink: 0 }}>
          <Text class="text-xs font-semibold text-primary-foreground">{count()}</Text>
        </View>
      </If>
    </Tab>
  );
}

function PhoneFolders() {
  const folders = () => profile().folders;
  return (
    <View class="flex-row flex-wrap gap-2">
      <FolderChip label="星标" picked={starView()} onPress={showStars} />
      <FolderChip label="全部" picked={activeFolder() === "" && !starView()} onPress={() => chooseFolder("")} />
      <FolderChip label="未分类" picked={activeFolder() === "-"} onPress={() => chooseFolder("-")} />
      {folders().map((folder) => (
        <FolderChip label={folder.name} picked={activeFolder() === folder.id} onPress={() => chooseFolder(folder.id)} />
      ))}
    </View>
  );
}

function FolderChip(props: { label: string; picked: boolean; onPress: () => void }) {
  return (
    <Button
      onPress={props.onPress}
      accessibilityLabel={
        props.label === "全部" ? "查看全部" : props.label === "未分类" ? "查看未分类" : props.label === "星标" ? "星标" : `查看文件夹 ${props.label}`
      }
      class={
        props.picked
          ? "rounded-full bg-primary px-3 py-1.5 active:opacity-80"
          : "rounded-full bg-secondary px-3 py-1.5 active:opacity-80"
      }
    >
      <Text
        class={props.picked ? "text-xs font-semibold text-primary-foreground" : "text-xs font-medium text-secondary-foreground"}
        numberOfLines={1}
      >
        {props.label}
      </Text>
    </Button>
  );
}

function Mark() {
  return (
    <View class="h-8 w-8 items-center justify-center rounded-[10px] bg-primary">
      <Text class="text-sm font-semibold text-primary-foreground">脉</Text>
    </View>
  );
}
