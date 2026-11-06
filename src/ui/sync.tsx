import { Clipboard, Text, View, effect, signal } from "nsh";
import { Button, Input, Radio, RadioGroup, Switch } from "nsh/base";
import { Core } from "theysay/native";
import { applyParsed, notify, profile } from "./model";
import {
  ensureSyncPrefs,
  pullSubscriptions,
  pushSubscriptions,
  type SyncConfig,
} from "./subscription-sync";
import { relativeTime } from "./text";

/** Settings block: off, iCloud, or a URL the user hosts. */
export function SubscriptionSync() {
  const url = signal("");
  const token = signal("");
  const pullOnOpen = signal(false);
  const status = signal("");
  const icloud = signal({ available: false, message: "" });
  effect(() => {
    const report = JSON.parse(Core.icloudStatus()) as { available?: boolean; message?: string };
    icloud.set({ available: report.available === true, message: report.message || "" });
    void ensureSyncPrefs().then((config) => {
      url.set(config.url);
      token.set(config.token);
      pullOnOpen.set(config.pullOnOpen);
    });
  });
  const mode = () => {
    const value = profile().sync?.mode;
    return value === "icloud" || value === "https" ? value : "off";
  };
  const current = (): SyncConfig => ({ url: url(), token: token(), pullOnOpen: pullOnOpen() });
  function remember(nextMode = mode()) {
    applyParsed(JSON.parse(Core.setSync(nextMode, url(), token(), pullOnOpen())));
  }
  const savedAt = () => profile().sync?.lastSyncMs ?? 0;
  return (
    <View class="gap-2">
      <Text class="text-xs font-medium text-muted-foreground">云端同步</Text>
      <Text class="text-xs leading-5 text-muted-foreground">
        同步的是订阅列表：文件夹，以及 X、Threads、Instagram、微博和 RSS 的关注。文章和已读留在这台设备上。
      </Text>
      <RadioGroup class="flex-row flex-wrap gap-2">
        <SyncChoice id="off" label="关闭" onPick={remember} />
        <SyncChoice id="icloud" label="iCloud" onPick={remember} />
        <SyncChoice id="https" label="自建服务器" onPick={remember} />
      </RadioGroup>
      <If when={mode() === "icloud"}>
        <Text class="text-xs leading-5 text-muted-foreground">
          {icloud().available ? "iCloud 可用。合并和上传使用跟自建服务器同一份订阅列表。" : icloud().message}
        </Text>
        <If when={savedAt() > 0}>
          <Text class="text-xs text-muted-foreground">上次同步 {relativeTime(savedAt())}</Text>
        </If>
        <If when={icloud().available}>
          <View class="flex-row flex-wrap gap-2">
            <Button
              onPress={() => {
                const env = applyParsed(JSON.parse(Core.icloudPull()));
                const message = env.ok
                  ? env.applied
                    ? "已从 iCloud 合并。"
                    : "订阅列表没有新的。"
                  : env.error || "没有从 iCloud 合并。";
                status.set(message);
                notify(message);
              }}
              accessibilityLabel="从 iCloud 合并"
              class="rounded-[10px] bg-primary px-3 py-2 active:opacity-80"
            >
              <Text class="text-sm font-semibold text-primary-foreground">从云端合并</Text>
            </Button>
            <Button
              onPress={() => {
                const env = applyParsed(JSON.parse(Core.icloudPush()));
                const message = env.ok ? "订阅列表已上传到 iCloud。" : env.error || "没有写进 iCloud。";
                status.set(message);
                notify(message);
              }}
              accessibilityLabel="上传到 iCloud"
              class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
            >
              <Text class="text-sm font-semibold text-secondary-foreground">上传订阅列表</Text>
            </Button>
          </View>
        </If>
      </If>
      <If when={mode() === "https"}>
        <Text class="text-xs leading-5 text-muted-foreground">
          两台设备填同一个 https 地址。地址要能 GET；上传还要它接受 PUT 或 POST。口令只存在这台设备的配置里，不会写进上传的列表。
        </Text>
        <Input
          value={url()}
          onChange={(text) => url.set(text)}
          onSubmit={() => remember()}
          placeholder="https://example.com/pulse-subscriptions.json"
          autoCapitalize="none"
          autoCorrect={false}
          class="rounded-xl border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground"
        />
        <Input
          value={token()}
          onChange={(text) => token.set(text)}
          placeholder="口令，可留空"
          autoCapitalize="none"
          autoCorrect={false}
          class="rounded-xl border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground"
        />
        <View class="flex-row flex-wrap gap-2">
          <Button
            onPress={() => {
              const next = Core.newSyncToken();
              token.set(next);
              applyParsed(JSON.parse(Core.setSync(mode(), url(), next, pullOnOpen())));
            }}
            accessibilityLabel="生成口令"
            class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
          >
            <Text class="text-sm font-semibold text-secondary-foreground">生成口令</Text>
          </Button>
          <Button
            onPress={() => {
              remember();
              notify("同步设置已记住");
            }}
            accessibilityLabel="记住同步设置"
            class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
          >
            <Text class="text-sm font-semibold text-secondary-foreground">记住</Text>
          </Button>
        </View>
        <Switch
          checked={pullOnOpen()}
          onChange={(on) => {
            pullOnOpen.set(on);
            applyParsed(JSON.parse(Core.setSync(mode(), url(), token(), on)));
          }}
          accessibilityLabel="打开时合并"
          class="flex-row-reverse items-center justify-between gap-3 rounded-[12px] bg-secondary px-3 py-2"
          trackClass="h-6 w-11 flex-row items-center rounded-full bg-muted p-0.5 checked:bg-primary"
          thumbClass="size-5 rounded-full bg-background shadow-sm checked:translate-x-5"
        >
          <Text class="min-w-0 flex-1 text-sm text-secondary-foreground">打开时合并</Text>
        </Switch>
        <If when={savedAt() > 0}>
          <Text class="text-xs text-muted-foreground">上次同步 {relativeTime(savedAt())}</Text>
        </If>
        <View class="flex-row flex-wrap gap-2">
          <Button
            onPress={() => {
              remember();
              void pullSubscriptions(current()).then((message) => {
                status.set(message);
                notify(message);
              });
            }}
            accessibilityLabel="从云端合并"
            class="rounded-[10px] bg-primary px-3 py-2 active:opacity-80"
          >
            <Text class="text-sm font-semibold text-primary-foreground">从云端合并</Text>
          </Button>
          <Button
            onPress={() => {
              remember();
              void pushSubscriptions(current()).then((message) => {
                status.set(message);
                notify(message);
              });
            }}
            accessibilityLabel="上传订阅列表"
            class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
          >
            <Text class="text-sm font-semibold text-secondary-foreground">上传订阅列表</Text>
          </Button>
          <Button
            onPress={() => {
              void Clipboard.writeText(Core.exportSubscriptions()).then(() => {
                const message = "订阅列表已复制。";
                status.set(message);
                notify(message);
              });
            }}
            accessibilityLabel="复制订阅列表"
            class="rounded-[10px] bg-secondary px-3 py-2 active:opacity-80"
          >
            <Text class="text-sm font-semibold text-secondary-foreground">复制订阅列表</Text>
          </Button>
        </View>
      </If>
      <If when={!!status()}>
        <Text class="text-xs text-muted-foreground">{status()}</Text>
      </If>
    </View>
  );
}

function SyncChoice(props: { id: string; label: string; onPick: (mode: string) => void }) {
  const selected = () => (profile().sync?.mode === "icloud" || profile().sync?.mode === "https" ? profile().sync.mode : "off") === props.id;
  return (
    <Radio
      checked={selected()}
      onChange={() => props.onPick(props.id)}
      accessibilityLabel={props.label}
      class="group justify-center rounded-full bg-secondary px-3 py-1.5 checked:bg-primary"
    >
      <Text class="text-xs text-secondary-foreground group-checked:font-semibold group-checked:text-primary-foreground">
        {props.label}
      </Text>
    </Radio>
  );
}
