import { Text } from "nsh";
import type { PostView, Segment } from "./model";

export function toneClass(tone: number) {
  switch (tone) {
    case 2:
      return "rounded-md bg-chart-2/20 px-1 text-sm font-semibold text-chart-2";
    case 3:
      return "rounded-md bg-chart-3/20 px-1 text-sm font-semibold text-chart-3";
    case 4:
      return "rounded-md bg-chart-4/20 px-1 text-sm font-semibold text-chart-4";
    case 5:
      return "rounded-md bg-chart-5/20 px-1 text-sm font-semibold text-chart-5";
    default:
      return "rounded-md bg-chart-1/20 px-1 text-sm font-semibold text-chart-1";
  }
}

export function RichText(props: { segments: Segment[]; dim?: boolean }) {
  return (
    <Text
      selectable
      class={props.dim ? "text-sm leading-[1.45] text-muted-foreground" : "text-sm leading-[1.45] text-card-foreground"}
    >
      {props.segments.map((part) =>
        part.kind === "ticker" ? (
          <Text selectable class={toneClass(part.tone)}>{` ${part.text} `}</Text>
        ) : (
          part.text
        ),
      )}
    </Text>
  );
}

export function dayLabel(timeMs: number) {
  return new Date(timeMs).toLocaleDateString("zh-CN", {
    month: "long",
    day: "numeric",
    weekday: "short",
  });
}

export function clockLabel(timeMs: number) {
  return new Date(timeMs).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
}

export function groupPosts(posts: PostView[]) {
  const rows: Array<{ kind: "day"; label: string } | { kind: "post"; post: PostView }> = [];
  let last = "";
  for (const post of posts) {
    const label = dayLabel(post.timeMs);
    if (label !== last) {
      rows.push({ kind: "day", label });
      last = label;
    }
    rows.push({ kind: "post", post });
  }
  return rows;
}

export function relativeTime(ms: number) {
  if (!ms) return "";
  const delta = Date.now() - ms;
  if (delta < 45_000) return "刚刚";
  const minutes = Math.round(delta / 60_000);
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.round(hours / 24);
  if (days === 1) return "昨天";
  if (days < 7) return `${days} 天前`;
  return dayLabel(ms);
}

export function freshLabel(ms: number) {
  if (!ms) return "";
  const when = relativeTime(ms);
  return when === "刚刚" ? "刚刚更新" : `${when}更新`;
}
