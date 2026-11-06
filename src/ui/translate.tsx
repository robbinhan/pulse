import { Core } from "theysay/native";
import { Text, View } from "nsh";
import { Button } from "nsh/base";
import { signal } from "nsh";
import { settingsOpen, type Segment } from "./model";

type Job = { state: "loading" | "done" | "error"; text: string };

const jobs = signal<Record<string, Job>>({});
const hidden = signal<Record<string, boolean>>({});

export function segmentText(segments: Segment[] | undefined): string {
  return (segments ?? []).map((part) => part.text).join("");
}

function deviceLocale(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().locale || "zh-Hans";
  } catch {
    return "zh-Hans";
  }
}

export function PostTranslation(props: { id: string; title: string; body: string; quote: string }) {
  const job = () => jobs()[props.id];
  const showing = () => job()?.state === "done" && !hidden()[props.id];
  function ask() {
    const current = jobs();
    if (current[props.id]?.state === "loading") return;
    if (current[props.id]?.state === "done") {
      hidden.set({ ...hidden(), [props.id]: false });
      return;
    }
    jobs.set({ ...current, [props.id]: { state: "loading", text: "" } });
    void Promise.resolve(Core.translate(props.id, props.title, props.body, props.quote, deviceLocale())).then((raw) => {
      let hit: { ok?: boolean; text?: string; error?: string } = {};
      try {
        hit = JSON.parse(String(raw ?? ""));
      } catch {
        hit = { ok: false, error: "翻译没有完成。" };
      }
      if (hit.ok && hit.text) {
        jobs.set({ ...jobs(), [props.id]: { state: "done", text: hit.text } });
        hidden.set({ ...hidden(), [props.id]: false });
      } else {
        jobs.set({ ...jobs(), [props.id]: { state: "error", text: hit.error || "翻译没有成功。检查地址或稍后再试。" } });
      }
    });
  }
  return (
    <View class="mt-2 gap-1.5">
      <If when={job()?.state === "loading"}>
        <Text class="text-xs text-muted-foreground">正在翻译…</Text>
      </If>
      <If when={showing()}>
        <Text selectable class="text-sm leading-[1.45] text-card-foreground">
          {job()?.text}
        </Text>
        <Button
          onPress={() => hidden.set({ ...hidden(), [props.id]: true })}
          class="self-start active:opacity-70"
        >
          <Text class="text-xs font-medium text-primary">显示原文</Text>
        </Button>
      </If>
      <If when={job()?.state === "error"}>
        <Text class="text-xs leading-5 text-destructive">{job()?.text}</Text>
      </If>
      <View class="flex-row flex-wrap gap-3">
        <If when={job()?.state !== "loading" && !showing()}>
          <Button onPress={ask} accessibilityLabel="翻译" class="active:opacity-70">
            <Text class="text-xs font-medium text-primary">{job()?.state === "done" ? "显示译文" : "翻译"}</Text>
          </Button>
        </If>
        <If when={job()?.state === "error"}>
          <Button onPress={() => settingsOpen.set(true)} class="active:opacity-70">
            <Text class="text-xs font-medium text-muted-foreground">去设置</Text>
          </Button>
        </If>
      </View>
    </View>
  );
}
