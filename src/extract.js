/* Original profile reader for Pulse (脉搏).
   It only reads the public DOM of the page it is already on and posts a
   JSON summary to the app. It does not read cookies or storage. */
(function () {
  var TOKEN = __NSH_TOKEN__;
  var DUMP = __NSH_DUMP__;
  var mark = "__theysayX_" + TOKEN;

  function post(body) {
    if (!window.nsh || typeof window.nsh.postMessage !== "function") return;
    try {
      body.token = TOKEN;
      body.href = String(location.href || "");
      window.nsh.postMessage(JSON.stringify(body));
    } catch (err) {}
  }

  function abs(href) {
    if (!href) return "";
    try {
      return new URL(href, location.href).href;
    } catch (err) {
      return "";
    }
  }

  function textOf(node) {
    if (!node) return "";
    return String(node.innerText || "")
      .replace(/[ \t]+\n/g, "\n")
      .replace(/\n{3,}/g, "\n\n")
      .trim();
  }

  function statusId(href) {
    var match = String(href || "").match(/\/status\/(\d+)/);
    return match ? match[1] : "";
  }

  function signedOut() {
    var path = String(location.pathname || "");
    if (/\/login|\/i\/flow\/|\/signup/.test(path)) return true;
    if (document.querySelector("article")) return false;
    var blob = textOf(document.body).slice(0, 900);
    return /Sign in to X|Sign in to Twitter|登录 X|注册 X/.test(blob);
  }

  function blocked() {
    if (document.querySelector('article[data-testid="tweet"], article')) return false;
    var blob = textOf(document.body).slice(0, 1500);
    return /verify you are human|are you a robot|rate limit|Just a moment|这些帖子已被保护|These posts are protected|This account doesn.?t exist|账号不存在/.test(
      blob
    );
  }

  function profileName() {
    var nodes = document.querySelectorAll('[data-testid="UserName"]');
    for (var i = 0; i < nodes.length; i++) {
      if (!nodes[i].closest("article")) {
        var line = textOf(nodes[i]).split("\n")[0] || "";
        return line.slice(0, 80);
      }
    }
    return "";
  }

  function mediaCandidates(node) {
    var values = [];
    function pushRaw(raw) {
      if (!raw) return;
      var bits = String(raw).split(",");
      for (var i = 0; i < bits.length; i++) {
        var piece = bits[i].trim().split(/\s+/)[0];
        if (piece) values.push(piece);
      }
    }
    if (!node) return values;
    if (node.currentSrc) pushRaw(node.currentSrc);
    if (node.getAttribute) {
      pushRaw(node.getAttribute("src"));
      pushRaw(node.getAttribute("data-src"));
      pushRaw(node.getAttribute("poster"));
      pushRaw(node.getAttribute("srcset"));
      pushRaw(node.getAttribute("data-srcset"));
      var style = node.getAttribute("style") || "";
      var re = /url\(\s*['"]?(https?:\/\/[^)'"]+)['"]?\s*\)/gi;
      var match;
      while ((match = re.exec(style))) values.push(match[1]);
    }
    return values;
  }

  function keepMedia(raw) {
    var href = abs(raw).split("#")[0];
    if (href.indexOf("https://") !== 0) return "";
    if (/profile_images|profile_banners|emoji|twemoji|blob:/.test(href)) return "";
    if (
      href.indexOf("https://pbs.twimg.com/media/") === 0 ||
      href.indexOf("https://pbs.twimg.com/ext_tw_video_thumb/") === 0 ||
      href.indexOf("https://pbs.twimg.com/amplify_video_thumb/") === 0 ||
      href.indexOf("https://pbs.twimg.com/tweet_video_thumb/") === 0 ||
      /cdninstagram\.com|\.fbcdn\.net|scontent\./.test(href)
    ) {
      return href;
    }
    return "";
  }

  function collectMedia(root, skip) {
    var images = [];
    if (!root || !root.querySelectorAll) return images;
    var nodes = root.querySelectorAll("img, video, source, [style*='url(']");
    for (var n = 0; n < nodes.length && images.length < 4; n++) {
      if (skip && skip.contains && skip.contains(nodes[n])) continue;
      var candidates = mediaCandidates(nodes[n]);
      for (var c = 0; c < candidates.length && images.length < 4; c++) {
        var src = keepMedia(candidates[c]);
        if (src && images.indexOf(src) === -1) images.push(src);
      }
    }
    return images;
  }

  function quoteBox(article, main, quoteNode) {
    if (!quoteNode) return null;
    var node = quoteNode;
    var box = quoteNode;
    while (node.parentElement && node.parentElement !== article && !node.parentElement.contains(main)) {
      node = node.parentElement;
      box = node;
    }
    return box;
  }

  function quoteAuthor(box) {
    if (!box) return "";
    var links = box.querySelectorAll("a[href]");
    for (var i = 0; i < links.length; i++) {
      var href = links[i].getAttribute("href") || "";
      if (href.indexOf("/status/") !== -1) continue;
      var match = href.match(/^\/([A-Za-z0-9_]{1,15})\/?$/);
      if (match) return match[1];
    }
    return "";
  }

  function articles() {
    var found = Array.prototype.slice.call(
      document.querySelectorAll('article[data-testid="tweet"]')
    );
    if (!found.length) {
      found = Array.prototype.slice.call(document.querySelectorAll("article"));
    }
    return found.filter(function (article) {
      return !article.parentElement || !article.parentElement.closest("article");
    });
  }

  function readArticle(article) {
    var texts = Array.prototype.slice.call(article.querySelectorAll('[data-testid="tweetText"]'));
    if (!texts.length) {
      var fallback = article.querySelector('[lang]');
      if (fallback) texts = [fallback];
    }
    var main = texts[0] || null;
    var quoteNode = null;
    for (var i = 1; i < texts.length; i++) {
      if (!main || !main.contains(texts[i])) {
        quoteNode = texts[i];
        break;
      }
    }
    var box = quoteBox(article, main, quoteNode);
    var time = article.querySelector("time");
    var datetime = time ? time.getAttribute("datetime") || "" : "";
    var timeLink = time ? time.closest("a") : null;
    var statusHref = abs(timeLink ? timeLink.getAttribute("href") : "");
    var id = statusId(statusHref);
    if (!id || !datetime) return null;

    var images = collectMedia(article, box);

    var linkUrl = "";
    var linkTitle = "";
    var card = article.querySelector('[data-testid="card.wrapper"]');
    if (card && !(box && box.contains(card))) {
      var anchor = card.querySelector("a[href]");
      var href = abs(anchor ? anchor.getAttribute("href") : "");
      if (href.indexOf("https://") === 0 && href.indexOf("/status/") === -1) linkUrl = href;
      var spans = card.querySelectorAll("span");
      for (var s = 0; s < spans.length; s++) {
        var piece = textOf(spans[s]);
        if (piece.length > linkTitle.length && piece.length <= 160 && piece.indexOf("http") !== 0) {
          linkTitle = piece;
        }
      }
    }

    var quote = null;
    if (quoteNode) {
      var qTime = box ? box.querySelector("time") : null;
      var qLink = qTime ? qTime.closest("a") : null;
      quote = {
        author: quoteAuthor(box),
        text: textOf(quoteNode).slice(0, 500),
        url: abs(qLink ? qLink.getAttribute("href") : ""),
      };
    }

    return {
      id: id,
      text: textOf(main).slice(0, 2000),
      time: datetime,
      url: statusHref,
      images: images,
      linkTitle: linkTitle,
      linkUrl: linkUrl,
      quote: quote,
    };
  }

  function pathHandle() {
    var match = String(location.pathname || "").match(/^\/([A-Za-z0-9_]{1,15})\/?$/);
    return match ? match[1] : "";
  }

  function scrape() {
    var host = String(location.hostname || "");
    if (host.indexOf("threads.") !== -1) scrapeThreads();
    else if (host.indexOf("instagram.") !== -1) scrapeInstagram();
    else if (host.indexOf("weibo.") !== -1) scrapeWeibo();
    else scrapeX();
  }

  function needsLogin(kind) {
    var path = String(location.pathname || "");
    var blob = textOf(document.body).slice(0, 900);
    if (kind === "threads") {
      if (/\/login|\/accounts\/login/.test(path)) return true;
      return /Log in to Threads|登录 Threads|Create a Threads profile/.test(blob) && !document.querySelector('a[href*="/post/"]');
    }
    if (/\/accounts\/login/.test(path)) return true;
    return /Log in to Instagram|登录 Instagram|手机号、用户名或邮箱/.test(blob) && !document.querySelector('a[href*="/p/"], a[href*="/reel/"]');
  }

  function postIdOf(href) {
    var match = String(href || "").match(/\/(?:post|t|p|reel)\/([A-Za-z0-9_-]{5,32})/);
    return match ? match[1] : "";
  }

  function dateStampOnly(text) {
    var value = String(text || "").trim();
    if (/^\d{1,2}\/\d{1,2}\/\d{2,4}$/.test(value)) return true;
    if (/^\d{4}-\d{2}-\d{2}$/.test(value)) return true;
    return /^\d{1,2}月\d{1,2}日/.test(value) && value.length <= 16;
  }

  function relativeTimeOnly(text) {
    var value = String(text || "").replace(/\s+/g, " ").trim();
    if (!value || value.length > 16) return false;
    if (/^(now|just now|today|昨天|刚刚)$/i.test(value)) return true;
    return /^\d+\s*(s|m|h|d|w|y|秒|分钟|分|小时|天|周|星期|年)$/i.test(value);
  }

  function noiseLine(line, author, handle) {
    var value = String(line || "").replace(/\s+/g, " ").trim();
    if (!value) return true;
    if (relativeTimeOnly(value)) return true;
    if (/^(translate|see translation|view translation|翻译|查看翻译|翻译帖子)$/i.test(value)) return true;
    if (/^(like|reply|repost|share|more|comment|send|赞|回复|转发|分享|评论)$/i.test(value)) return true;
    if (/^(pinned|repost(ed)?|已置顶|置顶|已转发)$/i.test(value)) return true;
    if (/^[\d,.]+([kmb])?$/.test(value.toLowerCase())) return true;
    if (/^[\d,.]+\s*(likes?|replies|reposts?|views?|comments?)$/i.test(value)) return true;
    if (author && value.toLowerCase() === author.toLowerCase()) return true;
    if (handle && value.replace(/^@/, "").toLowerCase() === handle.toLowerCase()) return true;
    return false;
  }

  function cleanCaption(raw, author, handle) {
    var lines = String(raw || "").split("\n");
    var kept = [];
    for (var i = 0; i < lines.length; i++) {
      if (!noiseLine(lines[i], author, handle)) kept.push(lines[i].trim());
    }
    var text = kept.join("\n").trim();
    if (relativeTimeOnly(text)) return "";
    return text.slice(0, 2000);
  }

  function idsIn(node) {
    var found = {};
    if (!node || !node.querySelectorAll) return found;
    var links = node.querySelectorAll("a[href]");
    for (var i = 0; i < links.length; i++) {
      var id = postIdOf(abs(links[i].getAttribute("href")));
      if (id) found[id] = 1;
    }
    return found;
  }

  function idCount(map) {
    var n = 0;
    for (var key in map) if (map.hasOwnProperty(key)) n++;
    return n;
  }

  // The timestamp link is not the post. Walk up until the parent holds
  // another post, and use the last node that still belongs to this one.
  function postRoot(anchor) {
    var node = anchor;
    var best = anchor.parentElement || anchor;
    for (var guard = 0; node.parentElement && node.parentElement !== document.body && guard < 16; guard++) {
      var parent = node.parentElement;
      if (idCount(idsIn(parent)) > 1) break;
      best = parent;
      node = parent;
    }
    return best;
  }

  function isAuthorPic(img) {
    var w = parseInt(img.getAttribute("width") || "0", 10);
    var h = parseInt(img.getAttribute("height") || "0", 10);
    if ((w && w <= 64) || (h && h <= 64)) return true;
    var link = img.closest ? img.closest("a[href]") : null;
    if (!link) return false;
    var href = abs(link.getAttribute("href")).split("?")[0];
    return /\/@[^/]+\/?$/.test(href) || /instagram\.com\/[^/]+\/?$/.test(href);
  }

  function postImages(root) {
    var images = [];
    if (!root || !root.querySelectorAll) return images;
    var nodes = root.querySelectorAll("picture img, picture source, img, video");
    for (var n = 0; n < nodes.length && images.length < 4; n++) {
      if (nodes[n].tagName === "IMG" && isAuthorPic(nodes[n])) continue;
      var candidates = mediaCandidates(nodes[n]);
      for (var c = 0; c < candidates.length && images.length < 4; c++) {
        var src = keepMedia(candidates[c]);
        if (src && images.indexOf(src) === -1) images.push(src);
      }
    }
    return images;
  }

  function altCaption(root) {
    if (!root || !root.querySelectorAll) return "";
    var best = "";
    var imgs = root.querySelectorAll("img[alt]");
    for (var i = 0; i < imgs.length; i++) {
      if (isAuthorPic(imgs[i])) continue;
      var alt = String(imgs[i].getAttribute("alt") || "").trim();
      if (alt.length < 2 || relativeTimeOnly(alt)) continue;
      if (/^(photo|image|图片|照片)( by .+)?$/i.test(alt)) continue;
      if (alt.length > best.length) best = alt;
    }
    return best.slice(0, 2000);
  }

  function captionOf(root, author, handle) {
    var best = "";
    if (root && root.querySelectorAll) {
      var nodes = root.querySelectorAll("[dir='auto'], span, p");
      for (var i = 0; i < nodes.length; i++) {
        if (nodes[i].closest && nodes[i].closest("time")) continue;
        if (nodes[i].querySelector && nodes[i].querySelector("time")) continue;
        var cleaned = cleanCaption(textOf(nodes[i]), author, handle);
        if (cleaned.length > best.length) best = cleaned;
      }
    }
    if (!best) best = altCaption(root);
    if (relativeTimeOnly(best)) return "";
    return best;
  }

  function beforePosts(node, firstPost) {
    if (!firstPost || !node || !firstPost.compareDocumentPosition) return true;
    return !(firstPost.compareDocumentPosition(node) & 4);
  }

  function headerProfile(handle) {
    var first = document.querySelector('a[href*="/post/"], a[href*="/p/"], a[href*="/reel/"]');
    var name = "";
    var avatar = "";
    var heads = document.querySelectorAll("h1, h2, [role='heading']");
    for (var i = 0; i < heads.length; i++) {
      if (!beforePosts(heads[i], first)) break;
      var line = textOf(heads[i]).split("\n")[0].trim();
      if (!line || relativeTimeOnly(line)) continue;
      if (handle && line.replace(/^@/, "").toLowerCase() === String(handle).toLowerCase()) continue;
      name = line.slice(0, 80);
      break;
    }
    var imgs = document.querySelectorAll("img");
    for (var j = 0; j < imgs.length; j++) {
      if (!beforePosts(imgs[j], first)) break;
      var src = "";
      var candidates = mediaCandidates(imgs[j]);
      for (var c = 0; c < candidates.length; c++) {
        src = keepMedia(candidates[c]);
        if (!src && /^https:/.test(abs(candidates[c]))) {
          var loose = abs(candidates[c]).split("#")[0];
          if (/cdninstagram\.com|\.fbcdn\.net|scontent\.|profile_images/.test(loose)) src = loose;
        }
        if (src) break;
      }
      if (src) {
        avatar = src;
        break;
      }
    }
    return { name: name, avatar: avatar };
  }

  function pageHandle(handleRe) {
    var handleMatch = String(location.pathname || "").match(handleRe);
    return handleMatch ? handleMatch[1] : "";
  }

  function collectFeed(idRe, handleRe) {
    var handle = pageHandle(handleRe);
    var header = headerProfile(handle);
    var anchors = document.querySelectorAll("a[href]");
    var seen = {};
    var posts = [];
    var found = 0;
    for (var i = 0; i < anchors.length && posts.length < 20; i++) {
      var href = abs(anchors[i].getAttribute("href"));
      var match = href.match(idRe);
      if (!match || seen[match[1]]) continue;
      seen[match[1]] = 1;
      var root = postRoot(anchors[i]);
      var timeEl = root.querySelector ? root.querySelector("time") : null;
      var datetime = timeEl ? timeEl.getAttribute("datetime") || "" : "";
      var author = "";
      var authorLink = root.querySelector ? root.querySelector('a[href*="/@"]') : null;
      if (authorLink) author = textOf(authorLink).split("\n")[0].trim();
      var text = captionOf(root, author || header.name, handle);
      var timeText = timeEl ? textOf(timeEl) : "";
      if (text && timeText && text === timeText) text = "";
      if (relativeTimeOnly(text) || dateStampOnly(text)) text = "";
      var images = postImages(root);
      found += images.length;
      if (!text && !images.length) continue;
      var item = {
        id: match[1],
        text: text,
        time: datetime,
        url: href.split("?")[0],
        images: images,
        linkTitle: "",
        linkUrl: "",
        quote: null,
      };
      posts.push(item);
    }
    var scripts = posts.length ? { count: 0, texts: [] } : collectScripts();
    var html = "";
    if (DUMP && document.documentElement) html = document.documentElement.outerHTML;
    var detail =
      "sjs=" +
      scripts.count +
      " matched=" +
      scripts.texts.length +
      " domPosts=" +
      posts.length +
      " images=" +
      found;
    console.log("[pulse] " + detail);
    var payload = {
      type: "embedded",
      state: posts.length || scripts.texts.length ? "ok" : "empty",
      handle: handle,
      displayName: header.name,
      avatar: header.avatar,
      detail: detail,
      scripts: scripts.texts,
      sjsCount: scripts.count,
      html: html,
      posts: posts,
    };
    post(payload);
    if (posts.length && String(location.hostname || "").indexOf("instagram.") !== -1) window.__nshIgDone = 1;
    inlineImages(payload, found);
  }

  function collectScripts() {
    var nodes = document.querySelectorAll('script[type="application/json"]');
    var texts = [];
    var bytes = 0;
    for (var i = 0; i < nodes.length; i++) {
      var text = nodes[i].textContent || "";
      if (
        text.indexOf("thread_items") < 0 &&
        text.indexOf("image_versions2") < 0 &&
        text.indexOf("carousel_media") < 0 &&
        text.indexOf("edge_owner_to_timeline_media") < 0 &&
        text.indexOf("edge_media_to_caption") < 0
      ) {
        continue;
      }
      if (text.length > 2500000) continue;
      if (bytes + text.length > 8000000) break;
      bytes += text.length;
      texts.push(text);
    }
    return { count: nodes.length, texts: texts };
  }

  function inlineImages(payload, found) {
    var urls = [];
    var list = payload.posts || [];
    for (var i = 0; i < list.length; i++) {
      var images = list[i].images || [];
      for (var n = 0; n < images.length && urls.length < 8; n++) {
        if (urls.indexOf(images[n]) === -1) urls.push(images[n]);
      }
    }
    if (!urls.length) {
      console.log("[pulse] images found=0 kept=0 inlined=0 failed=0");
      return;
    }
    var left = urls.length;
    var inlined = 0;
    var failed = 0;
    var inlines = [];
    function finish() {
      var line = "images found=" + found + " kept=" + urls.length + " inlined=" + inlined + " failed=" + failed;
      console.log("[pulse] " + line);
      post({
        type: "images",
        handle: payload.handle,
        displayName: payload.displayName,
        avatar: payload.avatar,
        detail: line,
        inlines: inlines,
        posts: [],
      });
    }
    for (var u = 0; u < urls.length; u++) {
      (function (url) {
        fetch(url, { credentials: "include" })
          .then(function (res) {
            if (!res.ok) {
              failed++;
              return null;
            }
            return res.blob();
          })
          .then(function (blob) {
            if (!blob) return null;
            if (blob.size < 32 || blob.size > 700000) {
              failed++;
              return null;
            }
            return new Promise(function (resolve) {
              var reader = new FileReader();
              reader.onload = function () {
                resolve(String(reader.result || ""));
              };
              reader.onerror = function () {
                resolve("");
              };
              reader.readAsDataURL(blob);
            });
          })
          .then(function (data) {
            if (data && data.indexOf("data:image/") === 0) {
              inlined++;
              inlines.push({ url: url, data: data });
            } else if (data === "") failed++;
          })
          .catch(function () {
            failed++;
          })
          .then(function () {
            left--;
            if (left === 0) finish();
          });
      })(urls[u]);
    }
  }

  function scrapeThreads() {
    if (needsLogin("threads")) {
      post({ type: "session", state: "signed-out", handle: "", displayName: "", detail: "", posts: [] });
      return;
    }
    collectFeed(/\/(?:post|t)\/([A-Za-z0-9_-]{5,32})/, /^\/@([A-Za-z0-9._]{1,30})/);
  }

  function scrapeInstagram() {
    if (window.__nshIgDone) return;
    if (needsLogin("instagram")) {
      post({ type: "session", state: "signed-out", handle: "", displayName: "", detail: "", posts: [] });
      return;
    }
    var handle = pageHandle(/^\/([A-Za-z0-9._]{1,30})\/?$/);
    if (handle && !window.__nshIgApi) {
      window.__nshIgApi = 1;
      var t0 = Date.now();
      fetch("https://www.instagram.com/api/v1/users/web_profile_info/?username=" + encodeURIComponent(handle), {
        credentials: "include",
        headers: { "X-IG-App-ID": "936619743392459", Accept: "application/json" },
      })
        .then(function (res) {
          return res.ok ? res.text() : "";
        })
        .then(function (raw) {
          var load = Date.now() - t0;
          if (raw && raw.indexOf("edge_owner_to_timeline_media") !== -1) {
            window.__nshIgDone = 1;
            post({
              type: "ig-api",
              state: "ok",
              handle: handle,
              displayName: "",
              avatar: "",
              detail: "api=web_profile_info",
              raw: raw,
              loadMs: load,
              posts: [],
            });
            return;
          }
          collectFeed(/\/(?:p|reel)\/([A-Za-z0-9_-]{5,32})/, /^\/([A-Za-z0-9._]{1,30})\/?$/);
        })
        .catch(function () {
          collectFeed(/\/(?:p|reel)\/([A-Za-z0-9_-]{5,32})/, /^\/([A-Za-z0-9._]{1,30})\/?$/);
        });
      return;
    }
    collectFeed(/\/(?:p|reel)\/([A-Za-z0-9_-]{5,32})/, /^\/([A-Za-z0-9._]{1,30})\/?$/);
  }

  function weiboUid() {
    var path = String(location.pathname || "");
    var match = path.match(/\/u\/(\d{5,16})/) || path.match(/\/(\d{5,16})\/?$/);
    return match ? match[1] : "";
  }

  function reportWeiboLogin() {
    if (window.__nshWbCfg) return;
    window.__nshWbCfg = 1;
    fetch("https://m.weibo.cn/api/config", { credentials: "include" })
      .then(function (res) {
        return res.ok ? res.json() : null;
      })
      .then(function (body) {
        var login = body && body.data ? body.data.login : null;
        if (login !== true && login !== false) return;
        post({
          type: "session",
          state: login === true ? "signed-in" : "signed-out",
          handle: "",
          displayName: "",
          detail: login === true ? "weibo login=true" : "weibo login=false",
          posts: [],
        });
      })
      .catch(function () {});
  }

  function scrapeWeibo() {
    if (window.__nshWbDone) return;
    var path = String(location.pathname || "");
    if (/login|signin/.test(path)) {
      post({ type: "session", state: "signed-out", handle: "", displayName: "", detail: "", posts: [] });
      return;
    }
    reportWeiboLogin();
    var uid = weiboUid();
    if (uid && !window.__nshWbApi) {
      window.__nshWbApi = 1;
      var t0 = Date.now();
      fetch("https://m.weibo.cn/api/container/getIndex?containerid=107603" + uid, { credentials: "include" })
        .then(function (res) {
          return res.ok ? res.text() : "";
        })
        .then(function (raw) {
          if (raw && raw.indexOf("\"mblog\"") !== -1) {
            window.__nshWbDone = 1;
            post({
              type: "weibo-api",
              state: "ok",
              handle: uid,
              displayName: "",
              avatar: "",
              detail: "api=getIndex",
              raw: raw,
              loadMs: Date.now() - t0,
              posts: [],
            });
            return;
          }
          scrapeWeiboDom(uid);
        })
        .catch(function () {
          scrapeWeiboDom(uid);
        });
      return;
    }
    scrapeWeiboDom(uid);
  }

  function scrapeWeiboDom(uid) {
    var posts = [];
    var seen = {};
    var links = document.querySelectorAll('a[href*="/detail/"], a[href*="/status/"]');
    for (var i = 0; i < links.length && posts.length < 12; i++) {
      var href = abs(links[i].getAttribute("href"));
      var match = href.match(/\/(?:detail|status)\/(\d{5,20})/);
      if (!match || seen[match[1]]) continue;
      seen[match[1]] = 1;
      var root = links[i].closest ? links[i].closest(".card, article, .weibo-main") || links[i].parentElement : links[i].parentElement;
      var text = root ? textOf(root).slice(0, 500) : "";
      if (!text) continue;
      posts.push({
        id: match[1],
        text: text,
        time: "",
        url: href.split("?")[0],
        images: postImages(root),
        linkTitle: "",
        linkUrl: "",
        quote: null,
      });
    }
    if (!posts.length) return;
    window.__nshWbDone = 1;
    post({
      type: "posts",
      state: "ok",
      handle: uid || "",
      displayName: "",
      avatar: "",
      detail: "dom=" + posts.length,
      posts: posts,
    });
  }

  function scrapeX() {
    if (signedOut()) {
      post({ type: "session", state: "signed-out", handle: "", displayName: "", detail: "", posts: [] });
      return;
    }
    if (blocked()) {
      post({
        type: "session",
        state: "blocked",
        handle: pathHandle(),
        displayName: profileName(),
        detail: "",
        posts: [],
      });
      return;
    }
    var posts = [];
    var list = articles();
    for (var i = 0; i < list.length && posts.length < 20; i++) {
      var postData = readArticle(list[i]);
      if (postData) posts.push(postData);
    }
    post({
      type: "posts",
      state: posts.length ? "ok" : "empty",
      handle: pathHandle(),
      displayName: profileName(),
      detail: "",
      posts: posts,
    });
  }

  function onX() {
    var host = String(location.hostname || "");
    return host.indexOf("x.com") !== -1 || host.indexOf("twitter.com") !== -1;
  }

  function reveal() {
    if (signedOut() || blocked()) return;
    try {
      // X virtualizes the timeline. Scrolling down unmounts the newest
      // tweets, and a later scrape then keeps yesterday's posts. Stay at
      // the top. Threads and Instagram still nudge downward to load more.
      if (onX()) {
        window.scrollTo(0, 0);
        return;
      }
      var top = Math.min(document.body ? document.body.scrollHeight : 0, 1800);
      window.scrollTo(0, top);
    } catch (err) {}
  }

  scrape();
  if (!window[mark]) {
    window[mark] = 1;
    if (onX()) {
      setTimeout(function () {
        reveal();
        scrape();
      }, 1000);
      setTimeout(function () {
        reveal();
        scrape();
      }, 3200);
      setTimeout(function () {
        try {
          window.scrollTo(0, 0);
        } catch (err) {}
        scrape();
      }, 9000);
    } else {
      // Poll the DOM instead of sleeping 1s, 3.2s, and 9s. Stop once an API
      // or the first post batch has been sent.
      var polls = 0;
      var timer = setInterval(function () {
        polls++;
        if (window.__nshIgDone || window.__nshWbDone || polls >= 16) {
          clearInterval(timer);
          return;
        }
        reveal();
        scrape();
      }, 250);
    }
  }
})();
