import { Storage, Text, View, WebView, cleanup, effect, mount, platform, setColorScheme } from "nsh";
import { browseAgent } from "./src/ui/web";
import { Core } from "theysay/native";
import { Overlays } from "./src/ui/overlays";
import { SessionProbe } from "./src/ui/probe";
import { Shell } from "./src/ui/shell";
import { Toaster } from "./src/ui/toast";
import { onScraperMessage, pauseScrapers, scraperToken, slots, startScraperLoop } from "./src/ui/scrapers";
import { SESSION_KEY, STORAGE_KEY, applyParsed, beginSessionProbe, loadSessions, loginOpen, noteSlow, probe, profile, readerUrl, ready, revision, scrollHandle } from "./src/ui/model";
import { maybeMorningBriefing } from "./src/ui/briefing";
import { pullSyncIfEnabled } from "./src/ui/subscription-sync";

function App() {
  effect(() => {
    let alive = true;
    // Before Storage reads the new bundle's file, and before any page opens.
    Core.prepareLegacy();
    Promise.all([Storage.getItem(STORAGE_KEY), Storage.getItem(SESSION_KEY)])
      .then(([raw, saved]) => {
        if (!alive) return;
        applyParsed(JSON.parse(Core.boot(raw ?? "")));
        loadSessions(saved || Core.legacySession() || null);
        applyTheme(profile().settings.theme);
        ready.set(true);
        // Saved "in" can be a guest Weibo from the last version. Check again.
        beginSessionProbe();
        void pullSyncIfEnabled();
        maybeMorningBriefing();
      })
      .catch(() => {
        if (alive) ready.set(true);
      });
    cleanup(() => {
      alive = false;
    });
  });

  effect(() => {
    if (!ready()) return;
    revision();
    applyTheme(profile().settings.theme);
    // The file is written off the UI thread. The dock badge is a count.
    const started = Date.now();
    Core.queueSave();
    Core.syncChrome();
    noteSlow("queue-save", started);
    // The file and the storage string are built off this thread. Poll until the
    // save that matches this revision is ready, then hand the string to storage.
    let alive = true;
    let tries = 0;
    let timer = setTimeout(pullExport, 40);
    function pullExport() {
      if (!alive) return;
      const json = Core.takeExport();
      if (json) {
        const writing = Date.now();
        void Storage.setItem(STORAGE_KEY, json);
        noteSlow("storage-export", writing);
        return;
      }
      tries += 1;
      if (tries < 200) timer = setTimeout(pullExport, 50);
    }
    cleanup(() => {
      alive = false;
      clearTimeout(timer);
    });
  });

  effect(() => {
    const timer = setInterval(() => {
      const raw = Core.pollNotice();
      if (!raw) return;
      const note = JSON.parse(raw) as { platform?: string; handle?: string };
      const platform = note.platform;
      if (platform !== "x" && platform !== "threads" && platform !== "instagram" && platform !== "weibo" && platform !== "rss") return;
      if (note.handle) scrollHandle.set(note.handle);
    }, 800);
    cleanup(() => clearInterval(timer));
  });

  effect(() => {
    pauseScrapers.set(loginOpen() || readerUrl() !== "" || probe() !== 0);
  });

  effect(() => {
    const stop = startScraperLoop();
    cleanup(stop);
  });

  return (
    <View
      class="flex-1 overflow-hidden bg-background"
      style={{
        paddingTop: platform.os === "ios" ? 0 : "env(safe-area-inset-top)",
        paddingRight: "env(safe-area-inset-right)",
        paddingBottom: platform.os === "ios" ? 0 : "env(safe-area-inset-bottom)",
        paddingLeft: "env(safe-area-inset-left)",
      }}
    >
      <If when={!ready()}>
        <View class="flex-1 items-center justify-center">
          <Text class="text-sm text-muted-foreground">正在打开本地配置…</Text>
        </View>
      </If>
      <If when={ready()}>
        <HiddenPages />
        <Shell />
        <Overlays />
        <Toaster />
        <SessionProbe />
      </If>
    </View>
  );
}

function HiddenPages() {
  return (
    <View
      pointerEvents="none"
      class="pointer-events-none"
      style={{ position: "absolute", left: 0, top: 0, width: 480, height: 1600, opacity: 0, zIndex: -10 }}
    >
      <Each of={slots()} key={(slot) => `${slot.platform}:${slot.index}:${slot.attempt}`}>
        {(slot) => (
          <WebView
            src={slot().url}
            userAgent={browseAgent(slot().platform)}
            injectedJavaScript={Core.extractScript(scraperToken(slot()))}
            onMessage={(event) => onScraperMessage(slot().platform, slot().index, event.data)}
            scrollable
            pointerEvents="none"
            class="pointer-events-none"
            style={{ width: 480, height: 1600 }}
          />
        )}
      </Each>
    </View>
  );
}

function applyTheme(theme: string) {
  setColorScheme(theme === "light" || theme === "dark" ? theme : "system");
}

mount(App);
