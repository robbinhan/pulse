import { Core } from "theysay/native";
import { Image, List, Text, View, cleanup } from "nsh";
import { Button } from "nsh/base";
import { effect, signal } from "nsh";
import { Glyph } from "./chrome";
import { activeFolder, applyParsed, noteSlow, notify, profile, readerUrl, revision, starView, type PlatformId } from "./model";
import { StarButton } from "./star";
import { postFp, reuseByKey } from "./stable";
import { RichText, relativeTime } from "./text";
import { PostTranslation, segmentText } from "./translate";

interface TimelinePost {
  id: string;
  segments: { kind: string; text: string; symbol: string; tone: number }[];
  timeMs: number;
  url: string;
  images: string[];
  linkTitle: string;
  linkUrl: string;
  quote: { author: string; url: string; segments: { kind: string; text: string; symbol: string; tone: number }[] } | null;
  unread: boolean;
}

interface TimelineItem {
  key: string;
  platform: string;
  handle: string;
  author: string;
  avatar?: string;
  cardIndex: number;
  post: TimelinePost;
}

interface TimelinePage {
  ok?: boolean;
  total?: number;
  items?: TimelineItem[];
}

const PAGE = 40;

function itemFp(item: TimelineItem): string {
  const avatar = (item.avatar || "").split("?")[0];
  return `${item.key}|${item.author}|${avatar}|${postFp(item.post)}`;
}

export function Timeline(props: { columns: number }) {
  const items = signal<TimelineItem[]>([]);
  const total = signal(0);
  const loading = signal(false);
  let loaded = PAGE;
  let seenFocus = "";
  let seenFolder = "";
  let seenStars = false;
  const kept = new Map<string, TimelineItem>();
  let order: TimelineItem[] = [];
  function publish(next: TimelineItem[], pageTotal: number) {
    const adopted = reuseByKey(kept, order, next, (item) => item.key, itemFp);
    total.set(pageTotal);
    loaded = Math.max(PAGE, adopted.items.length);
    if (!adopted.changed) return;
    order = adopted.items;
    items.set(adopted.items);
  }
  effect(() => {
    const focus = profile().focus;
    const folder = activeFolder();
    const stars = starView();
    revision();
    if (focus !== seenFocus || folder !== seenFolder || stars !== seenStars) {
      seenFocus = focus;
      seenFolder = folder;
      seenStars = stars;
      loaded = PAGE;
      kept.clear();
      order = [];
    }
    const started = Date.now();
    const page = JSON.parse(stars ? Core.starredTimeline(0, loaded) : Core.timeline(focus, 0, loaded, folder)) as TimelinePage;
    noteSlow("timeline", started);
    publish(page.items ?? [], page.total ?? 0);
  });
  function more() {
    if (loading()) return;
    const have = items().length;
    if (have === 0 || have >= total()) return;
    loading.set(true);
    const folder = activeFolder();
    const page = JSON.parse(
      starView() ? Core.starredTimeline(have, PAGE) : Core.timeline(profile().focus, have, PAGE, folder),
    ) as TimelinePage;
    const extra = page.items ?? [];
    if (extra.length > 0) publish([...items(), ...extra], page.total ?? total());
    else total.set(page.total ?? total());
    loading.set(false);
  }
  return (
    <List
      each={items()}
      keyExtractor={(item) => item.key}
      numColumns={props.columns}
      estimatedItemSize={180}
      onLoadMore={more}
      style={{ flex: 1, minHeight: 0 }}
      contentClass="gap-3 p-4"
      empty={<TimelineEmpty />}
    >
      {(item) => <StreamRow item={item} />}
    </List>
  );
}

function StreamRow(props: { item: () => TimelineItem }) {
  const row = () => props.item();
  function openPost() {
    const current = row();
    if (current.post.unread) {
      applyParsed(JSON.parse(Core.markPost(current.platform, current.cardIndex, current.post.id)));
    }
    if (!current.post.url) {
      notify("这条帖子没有链接");
      return;
    }
    readerUrl.set(current.post.url);
  }
  const platformId = (): PlatformId => {
    const id = row().platform;
    return id === "threads" || id === "instagram" || id === "weibo" || id === "rss" ? id : "x";
  };
  return (
    <View class="min-w-0 flex-1 overflow-hidden rounded-[16px] border border-border bg-card px-3 py-3">
      <View class="mb-2 flex-row items-center gap-2">
        <If when={!!row().avatar}>
          <StreamAvatar src={row().avatar || ""} platform={platformId()} />
        </If>
        <If when={!row().avatar}>
          <Glyph platform={platformId()} size={22} />
        </If>
        <Text class="min-w-0 flex-1 text-sm font-semibold text-card-foreground" numberOfLines={1}>
          {row().author}
        </Text>
        <Text class="text-xs text-muted-foreground">{relativeTime(row().post.timeMs)}</Text>
      </View>
      <View selectable>
        <View
          class={
            row().post.unread
              ? "mb-1.5 h-1 w-6 rounded-full bg-primary"
              : "mb-1.5 h-1 w-6 rounded-full bg-transparent"
          }
        />
        <If when={row().platform === "rss" && !!row().post.linkTitle}>
          <Text selectable class="mb-1 text-[15px] font-semibold leading-5 text-card-foreground">
            {row().post.linkTitle}
          </Text>
        </If>
        <RichText segments={row().post.segments} />
      </View>
      <If when={row().post.images.length > 0}>
        <Button onPress={openPost} accessibilityLabel="打开图片" class="active:opacity-90">
          <Image
            src={row().post.images[0]}
            resizeMode="cover"
            style={{ width: "100%", height: 160, borderRadius: 12, marginTop: 8 }}
          />
        </Button>
      </If>
      <View class="mt-2 flex-row items-center gap-3">
        <Button onPress={openPost} accessibilityLabel="打开帖子" class="active:opacity-70">
          <Text class="text-xs font-medium text-primary">打开</Text>
        </Button>
        <StarButton platform={row().platform} id={row().post.id} on={!!row().post.starred} />
      </View>
      <PostTranslation
        id={`${row().platform}:${row().handle}:${row().post.id}`}
        title={row().platform === "rss" ? row().post.linkTitle : ""}
        body={segmentText(row().post.segments)}
        quote={segmentText(row().post.quote?.segments)}
      />
    </View>
  );
}

function StreamAvatar(props: { src: string; platform: PlatformId }) {
  const local = signal("");
  effect(() => {
    const src = props.src;
    let alive = true;
    void Core.loadMedia(src, props.platform).then((uri) => {
      if (alive && uri) local.set(uri);
    });
    cleanup(() => {
      alive = false;
    });
  });
  return (
    <Image
      src={local() || props.src}
      resizeMode="cover"
      style={{ width: 22, height: 22, borderRadius: 11 }}
    />
  );
}

function TimelineEmpty() {
  const focus = () => profile().focus;
  const line = () => {
    if (focus() === "rss") return "这个筛选下还没有文章。空订阅可以在管理订阅里清掉。";
    if (focus() === "all") return "关注账号或加上订阅之后，新内容会按时间排在这里。";
    return "这个平台还没有帖子。登录在设置里，添加账号在看板上。";
  };
  return (
    <View class="items-center px-6 py-16">
      <Text class="text-sm font-medium text-foreground">还没有帖子</Text>
      <Text class="mt-1 text-center text-xs leading-5 text-muted-foreground">{line()}</Text>
    </View>
  );
}
