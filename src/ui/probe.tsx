import { Core } from "theysay/native";
import { View, WebView, cleanup, effect, signal, untracked } from "nsh";
import { advanceProbe, probe, probePlatform, rememberSession, type PlatformId } from "./model";
import { browseAgent } from "./web";

const PROBE_MS = 7000;

const PROBE_SCRIPT = `(function () {
  function cookieHas(name) {
    var parts = String(document.cookie || "").split(";");
    for (var i = 0; i < parts.length; i++) {
      var piece = parts[i].replace(/^\\s+/, "");
      if (piece.indexOf(name + "=") === 0 && piece.length > name.length + 1) return true;
    }
    return false;
  }
  function scriptHasViewer() {
    var nodes = document.scripts || [];
    for (var i = 0; i < nodes.length && i < 80; i++) {
      var text = nodes[i].textContent || "";
      if (!text || text.length > 400000) continue;
      if (text.indexOf("viewer") === -1 && text.indexOf("viewerId") === -1) continue;
      if (/"viewerId":"\\d{3,}"/.test(text)) return true;
      if (/"viewer":\\{"user":\\{"id":"?\\d{3,}/.test(text)) return true;
    }
    return false;
  }
  function loginWall() {
    var path = String(location.pathname || "");
    if (/\\/accounts\\/login|\\/login|\\/i\\/flow\\//.test(path)) return true;
    var blob = String((document.body && document.body.innerText) || "").slice(0, 2500);
    return /Continue with Instagram|Log in with Instagram|Log into Threads|使用 Instagram 账号继续|使用 Instagram 登录|登录 Threads|Sign in to X|登录 X/.test(blob);
  }
  function post(marker, viewer) {
    if (!window.nsh || typeof window.nsh.postMessage !== "function") return;
    var wall = loginWall();
    if (viewer || marker) wall = false;
    try {
      window.nsh.postMessage(JSON.stringify({
        type: "session-check",
        href: String(location.href || ""),
        marker: !!marker,
        wall: wall,
        viewer: !!viewer
      }));
    } catch (err) {}
  }
  function askWeibo() {
    if (window.__nshWbProbe) return;
    window.__nshWbProbe = 1;
    fetch("https://m.weibo.cn/api/config", { credentials: "include" })
      .then(function (res) {
        return res.ok ? res.json() : null;
      })
      .then(function (body) {
        var login = body && body.data ? body.data.login : null;
        if (login !== true && login !== false) {
          window.__nshWbProbe = 0;
          return;
        }
        if (!window.nsh || typeof window.nsh.postMessage !== "function") return;
        window.nsh.postMessage(JSON.stringify({
          type: "session-check",
          href: String(location.href || ""),
          marker: false,
          weiboLogin: login === true
        }));
      })
      .catch(function () {
        window.__nshWbProbe = 0;
      });
  }
  function look() {
    var host = String(location.hostname || "");
    var viewer = scriptHasViewer() || cookieHas("ds_user_id") || cookieHas("sessionid");
    var marker = false;
    if (host.indexOf("threads.") !== -1) {
      marker = !!(
        document.querySelector('[aria-label="New thread"]') ||
        document.querySelector('[aria-label="新串文"]') ||
        document.querySelector('[aria-label="Create"]') ||
        document.querySelector('[aria-label="Profile"]') ||
        document.querySelector('[aria-label="主页"]') ||
        document.querySelector('div[role="textbox"]') ||
        document.querySelector('a[href="/activity"]') ||
        document.querySelector('[aria-label="Search"]')
      );
    } else if (host.indexOf("weibo.") !== -1) {
      askWeibo();
      return;
    } else if (host.indexOf("instagram.") !== -1) {
      marker = !!(
        document.querySelector('a[href="/direct/inbox/"]') ||
        document.querySelector('a[href*="/direct/inbox"]') ||
        document.querySelector('[aria-label="New post"]') ||
        document.querySelector('[aria-label="新帖子"]') ||
        document.querySelector('a[href="/explore/"]') ||
        document.querySelector('[aria-label="Home"]')
      );
    } else {
      marker = !!(
        document.querySelector('[data-testid="SideNav_AccountSwitcher_Button"]') ||
        document.querySelector('[data-testid="tweetTextarea_0"]') ||
        document.querySelector('[data-testid="AppTabBar_Home_Link"]') ||
        document.querySelector('a[href="/compose/post"]') ||
        document.querySelector('[data-testid="AppTabBar_Profile_Link"]')
      );
    }
    post(marker, viewer);
  }
  look();
  setTimeout(look, 700);
  setTimeout(look, 1800);
  setTimeout(look, 3600);
  setTimeout(look, 5500);
})();`;

export const lastProbeUrl = signal("https://x.com/home");

export function SessionProbe() {
  effect(() => {
    const id = probe();
    const platform = probePlatform();
    if (!id) return;
    lastProbeUrl.set(Core.homeUrl(platform));
    const timer = setTimeout(() => {
      untracked(() => {
        if (probe() !== id) return;
        settle(platform, lastProbeUrl(), false, true, false, false);
      });
    }, PROBE_MS);
    cleanup(() => clearTimeout(timer));
  });

  return (
    <View
      pointerEvents="none"
      class="pointer-events-none"
      style={{ position: "absolute", left: -4200, top: 0, width: 480, height: 900, zIndex: -10 }}
    >
      <If when={probe() !== 0 && probePlatform() === "x"}>
        <ProbePage platform="x" src="https://x.com/home" />
      </If>
      <If when={probe() !== 0 && probePlatform() === "threads"}>
        <ProbePage platform="threads" src="https://www.threads.com/" />
      </If>
      <If when={probe() !== 0 && probePlatform() === "instagram"}>
        <ProbePage platform="instagram" src="https://www.instagram.com/" />
      </If>
      <If when={probe() !== 0 && probePlatform() === "weibo"}>
        <ProbePage platform="weibo" src="https://m.weibo.cn/" />
      </If>
    </View>
  );
}

function ProbePage(props: { platform: PlatformId; src: string }) {
  return (
    <WebView
      src={props.src}
      userAgent={browseAgent(props.platform)}
      injectedJavaScript={PROBE_SCRIPT}
      onNavigationStateChange={(nav) => onProbeNavigation(props.platform, nav.url || "")}
      onMessage={(event) => onProbeMessage(props.platform, event.data)}
      pointerEvents="none"
      class="pointer-events-none"
      style={{ width: 480, height: 900 }}
    />
  );
}

function settle(
  platform: PlatformId,
  url: string,
  hasMarker: boolean,
  final: boolean,
  wall: boolean,
  viewer: boolean,
) {
  const verdict = Core.sessionFromSignals(platform, url, hasMarker, wall, viewer);
  if (verdict === "in" || verdict === "out") {
    rememberSession(platform, verdict);
    advanceProbe();
    return;
  }
  // No marker and no login wall. A timeout must not turn a saved "in" into "out".
  if (final) advanceProbe();
}

function onProbeNavigation(platform: PlatformId, url: string) {
  lastProbeUrl.set(url);
  settle(platform, url, false, false, false, false);
}

function onProbeMessage(platform: PlatformId, data: string) {
  let body: { type?: string; href?: string; marker?: boolean; wall?: boolean; viewer?: boolean; weiboLogin?: boolean };
  try {
    body = JSON.parse(data);
  } catch {
    return;
  }
  if (body.type !== "session-check") return;
  if (platform === "weibo") {
    const state = Core.weiboConfigState(JSON.stringify({ data: { login: body.weiboLogin } }));
    if (state === "in" || state === "out") {
      rememberSession(platform, state);
      advanceProbe();
    }
    return;
  }
  const href = body.href || lastProbeUrl();
  lastProbeUrl.set(href);
  settle(platform, href, !!body.marker, false, !!body.wall, !!body.viewer);
}
