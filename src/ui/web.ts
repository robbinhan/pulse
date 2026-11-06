import { platform } from "nsh";

/** Desktop Safari. Instagram and Threads serve a blank page to WKWebView's own user agent on macOS. */
export const DESKTOP_SAFARI =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";

/**
 * Mobile Safari. iOS WKWebView's own agent omits `Safari`, so x.com/login answers
 * `302` to `x-safari-https://…`. That scheme is not http(s); the web view cancels
 * it and the sheet stays white (`帧框加载已中断`). Instagram and Threads paint a
 * blank document for the same agent. Cookies stay in this app's web store.
 */
export const IOS_SAFARI =
  "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1";

/**
 * User agent for a platform page.
 * iOS uses Mobile Safari for every site, including Instagram and Threads fetches
 * after the in-app login. macOS uses desktop Safari for those two.
 */
export function browseAgent(kind: string): string | undefined {
  // Mac: desktop Safari, or Instagram and Threads paint a blank embedded page.
  // iPhone: Mobile Safari. A Macintosh agent on an iPhone is classified as an
  // iPad (touch points > 1) and the login document stays empty.
  if (platform.os === "ios") return IOS_SAFARI;
  if (kind === "threads" || kind === "instagram") return DESKTOP_SAFARI;
  if (kind === "x") return undefined;
  if (kind === "weibo") return IOS_SAFARI;
  return DESKTOP_SAFARI;
}

/**
 * iOS Instagram and Threads login. Their phone pages are a splash until a
 * script runs; that script does not draw a form inside WKWebView (build 2
 * with a Safari agent, and build 3 with a desktop agent, both stayed white).
 * This document is same-origin with the site (`baseUrl`) and posts to the
 * site's own login endpoint, so `sessionid` lands in this app's web store.
 */
export const META_SESSION_HTML = `<!DOCTYPE html><html><head><meta charset="utf-8"></head><body><script>
(function () {
  var ident = "";
  function post(body) {
    try { if (window.nsh && window.nsh.postMessage) window.nsh.postMessage(JSON.stringify(body)); } catch (err) {}
  }
  function cookie(name) {
    var match = document.cookie.match(new RegExp("(?:^|; )" + name + "=([^;]*)"));
    return match ? decodeURIComponent(match[1]) : "";
  }
  function enc(password) {
    return "#PWD_INSTAGRAM_BROWSER:0:" + Date.now() + ":" + password;
  }
  async function csrf(page) {
    var token = cookie("csrftoken");
    if (token) return token;
    await fetch(page, { credentials: "include" });
    return cookie("csrftoken");
  }
  async function send(url, page, appId, fields) {
    var token = await csrf(page);
    var body = new URLSearchParams(fields);
    var res = await fetch(url, {
      method: "POST",
      credentials: "include",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
        "X-CSRFToken": token,
        "X-Requested-With": "XMLHttpRequest",
        "X-IG-App-ID": appId,
        "Referer": page
      },
      body: body.toString()
    });
    var text = await res.text();
    try { return JSON.parse(text); } catch (err) { return { status: "fail", message: text.slice(0, 180) }; }
  }
  window.addEventListener("message", function (event) {
    var data = event.data;
    try { if (typeof data === "string") data = JSON.parse(data); } catch (err) { return; }
    if (!data || !data.kind) return;
    var root = data.origin;
    var page = data.page;
    var appId = data.appId;
    var username = data.username || "";
    if (data.kind === "code") {
      send(root + "/api/v1/web/accounts/login/ajax/two_factor/", page, appId, {
        username: username,
        verificationCode: data.code || "",
        identifier: ident,
        trust_this_device: "true",
        queryParams: "{}"
      }).then(function (body) {
        if (body && body.authenticated) post({ ok: true });
        else post({ ok: false, message: (body && body.message) || "验证码不对" });
      }).catch(function () { post({ ok: false, message: "网络没有连上" }); });
      return;
    }
    send(root + "/api/v1/web/accounts/login/ajax/", page, appId, {
      username: username,
      enc_password: enc(data.password || ""),
      queryParams: "{}",
      optIntoOneTap: "false"
    }).then(function (body) {
      var info = body && body.two_factor_info;
      if (body && body.two_factor_required && info) {
        ident = info.two_factor_identifier || "";
        post({ ok: false, twoFactor: true, message: "再输入验证码" });
        return;
      }
      if (body && body.authenticated) {
        post({ ok: true });
        return;
      }
      if (body && body.checkpoint_url) {
        post({ ok: false, checkpoint: body.checkpoint_url, message: "还要再验证一次" });
        return;
      }
      var missing = body && body.user === false;
      post({ ok: false, message: missing ? "没有这个账号" : ((body && body.message) || "账号或密码不对") });
    }).catch(function () { post({ ok: false, message: "网络没有连上" }); });
  });
  post({ ready: true });
})();
</script></body></html>`;

/** Login sheet. Desktop Weibo needs desktop Safari so the passport QR image is served. */
/** Looks up a nickname inside the logged-in m.weibo.cn web view. */
export function weiboResolveScript(query: string) {
  const q = JSON.stringify(query);
  return `(function () {
  var q = ${q};
  function post(body) {
    body.type = "weibo-resolve";
    body.q = q;
    try {
      if (window.nsh && window.nsh.postMessage) window.nsh.postMessage(JSON.stringify(body));
    } catch (err) {}
  }
  var path = String(location.pathname || "");
  var uid = (path.match(/\\/(?:u|profile)\\/(\\d{5,16})/) || [])[1] || "";
  if (uid) {
    fetch("https://m.weibo.cn/api/container/getIndex?containerid=107603" + uid, { credentials: "include" })
      .then(function (res) { return res.ok ? res.text() : ""; })
      .then(function (raw) { post({ via: "profile", uid: uid, raw: raw || "" }); })
      .catch(function () { post({ via: "redirect", uid: uid, raw: "" }); });
    return;
  }
  if (!q) {
    post({ via: "search", raw: "" });
    return;
  }
  var container = "100103type=3&q=" + q;
  fetch("https://m.weibo.cn/api/container/getIndex?containerid=" + encodeURIComponent(container) + "&page_type=searchall", { credentials: "include" })
    .then(function (res) { return res.ok ? res.text() : ""; })
    .then(function (raw) { post({ via: "search", raw: raw || "" }); })
    .catch(function () { post({ via: "search", raw: "" }); });
})();`;
}

export function loginAgent(kind: string): string | undefined {
  if (platform.os === "ios") return IOS_SAFARI;
  if (kind === "weibo" || kind === "threads" || kind === "instagram") return DESKTOP_SAFARI;
  return browseAgent(kind);
}

/**
 * Keeps Weibo passport popups in this web view, logs images that fail
 * (the QR is https://v2.qr.weibo.cn/...), and after SSO lands on weibo.com
 * continues to m.weibo.cn so `.weibo.cn` receives SUB. `api/config` then
 * reports whether that session exists.
 */
export const WEIBO_LOGIN_SCRIPT = `(function () {
  function post(body) {
    try {
      if (window.nsh && window.nsh.postMessage) window.nsh.postMessage(JSON.stringify(body));
    } catch (err) {}
  }
  window.open = function (url) {
    if (url) location.href = String(url);
    return null;
  };
  document.addEventListener("click", function (event) {
    var node = event.target;
    while (node && node.tagName !== "A") node = node.parentElement;
    if (!node || !node.href) return;
    if (node.target === "_blank" || node.target === "_new") {
      event.preventDefault();
      location.href = node.href;
    }
  }, true);
  document.addEventListener("submit", function (event) {
    var form = event.target;
    if (form && form.target === "_blank") form.target = "_self";
  }, true);
  window.addEventListener("error", function (event) {
    var el = event.target;
    if (!el || el === window) return;
    var src = el.currentSrc || el.src || el.href || "";
    if (!src || String(src).indexOf("http") !== 0) return;
    post({ type: "weibo-login", event: "fail", tag: el.tagName || "", src: String(src) });
  }, true);
  function tick() {
    var host = String(location.hostname || "");
    var passport = host.indexOf("passport.") === 0 || host.indexOf("login.sina.com") !== -1;
    var desktopHome = /(^|\\.)weibo\\.com$/.test(host) && !passport;
    if (desktopHome && (location.pathname === "/" || location.pathname === "/home" || location.pathname === "/home/")) {
      location.replace("https://m.weibo.cn/");
      return;
    }
    if (host !== "m.weibo.cn" && host !== "weibo.cn") return;
    fetch("https://m.weibo.cn/api/config", { credentials: "include" })
      .then(function (res) { return res.ok ? res.json() : null; })
      .then(function (body) {
        var login = body && body.data ? body.data.login : null;
        if (login !== true && login !== false) return;
        post({ type: "weibo-login", event: "config", login: login === true, href: String(location.href || "") });
      })
      .catch(function () {});
  }
  if (document.readyState === "complete") tick();
  else window.addEventListener("load", tick);
})();`;
