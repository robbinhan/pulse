import { Text, View, cleanup, effect, platform, signal } from "nsh";
import { Toast, ToastManager, ToastStack, type ToastEntry } from "nsh/base";

const manager = new ToastManager();
const shown = signal<ToastEntry[]>([]);
const expanded = signal(false);
let seq = 0;

function sync() {
  shown.set(manager.visible(3));
}

function titleOf(entry: ToastEntry) {
  const value = entry.value as { title?: string } | null;
  return value && value.title ? value.title : "";
}

/** A short message over the window. The stack hides it after a few seconds. */
export function pushToast(title: string) {
  seq += 1;
  manager.push(`t${seq}`, { title }, { timeout: 3600 });
  sync();
}

export function Toaster() {
  effect(() => {
    const timer = setInterval(() => {
      if (manager.advance(expanded()).changed) sync();
    }, 50);
    cleanup(() => clearInterval(timer));
  });

  return (
    <View
      pointerEvents="box-none"
      class="absolute inset-x-0 bottom-0 items-center"
      style={{
        paddingBottom:
          platform.os === "ios" ? "calc(env(safe-area-inset-bottom) + 88px)" : 24,
      }}
    >
      <ToastStack
        placement="bottomCenter"
        class="w-80"
        onExpandedChange={(open) => expanded.set(open)}
        onCancel={() => {
          const live = shown().filter((item) => item.status !== "ending");
          const newest = live[live.length - 1];
          if (!newest) return;
          manager.dismiss(newest.id);
          sync();
        }}
      >
        <Each of={shown()} key={(item) => item.id}>
          {(item) => (
            <Toast
              transitionStatus={item().status}
              accessibilityLabel={titleOf(item())}
              swipeDirection="up"
              onDismissRequest={() => {
                manager.dismiss(item().id);
                sync();
              }}
              class="rounded-full bg-foreground px-4 py-2 shadow-sm"
              startingClass="opacity-0 translate-y-2"
              endingClass="opacity-0"
            >
              <Text class="text-sm text-background">{titleOf(item())}</Text>
            </Toast>
          )}
        </Each>
      </ToastStack>
    </View>
  );
}
