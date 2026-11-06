import { Text, View } from "nsh";
import { Button, Tooltip } from "nsh/base";

export function Glyph(props: { platform: string; size?: number }) {
  const size = props.size ?? 28;
  const kind =
    props.platform === "threads"
      ? "threads"
      : props.platform === "instagram"
        ? "instagram"
        : props.platform === "weibo"
          ? "weibo"
          : props.platform === "rss"
            ? "rss"
            : "x";
  const label = kind === "threads" ? "@" : kind === "instagram" ? "Ig" : kind === "weibo" ? "微" : kind === "rss" ? "订" : "X";
  if (kind === "instagram") {
    return (
      <View
        class="items-center justify-center bg-linear-to-br from-chart-2 via-chart-4 to-chart-5"
        style={{ width: size, height: size, borderRadius: size > 24 ? 10 : 8 }}
      >
        <Text class="font-semibold text-white" style={{ fontSize: size > 24 ? 12 : 10 }}>
          {label}
        </Text>
      </View>
    );
  }
  const dark = kind === "threads" || kind === "x" || kind === "weibo" || kind === "rss";
  return (
    <View
      class="items-center justify-center"
      style={{
        width: size,
        height: size,
        borderRadius: size > 24 ? 10 : 8,
        backgroundColor: dark ? "#111111" : "#18181b",
      }}
    >
      <Text class="font-semibold text-white" style={{ fontSize: size > 24 ? 13 : 11 }}>
        {label}
      </Text>
    </View>
  );
}

export function IconButton(props: { label: string; glyph: string; onPress: () => void; busy?: boolean }) {
  return (
    <Button
      onPress={props.onPress}
      accessibilityLabel={props.label}
      tooltip={
        <Tooltip class="rounded-lg border border-border bg-popover px-2 py-1">
          <Text class="text-xs text-popover-foreground">{props.label}</Text>
        </Tooltip>
      }
      class="h-8 w-8 items-center justify-center rounded-[8px] hover:bg-accent active:opacity-70"
    >
      <Text class="text-sm text-foreground">{props.busy ? "…" : props.glyph}</Text>
    </Button>
  );
}
