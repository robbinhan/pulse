# Pulse（脉搏）

用 [NSH 0.3.0](https://github.com/longbridge/nsh/releases/tag/v0.3.0) 写的原生阅读器。这一版接 **X**、**Threads**、**Instagram**、**微博** 和 **RSS**。打开就是 **动态**：各平台的帖子按时间排在一起，新的在上。侧栏是文件夹：全部、未分类，以及你建的目录。X、Threads、Instagram、微博和 RSS 都是订阅的来源，可以放进同一个文件夹。登录在设置里。管理订阅和 OPML 仍用来管理列表。**看板** 把原来的两列账号格子留着，不是默认首页。窄窗口和 iPhone 是一列。

这是对照 [TheySay](https://github.com/laike9m/TheySay) 的行为自己写的，**没有拷贝它的源码或抓取脚本**。TheySay 本体使用 PolyForm Noncommercial 1.0.0；本仓库不包含那份代码。窗口标题、macOS 的 `Pulse.app` 和 iPhone 主屏幕上的名字都是 **Pulse**，应用里写的是 **脉搏**。

界面是 TSX，时间线的合并、未读、$TICKER、屏蔽词、导入校验和「同时开几个网页」都在 Rust 里，并且有单测。

## 在 Apple 芯片的 Mac 上打包

需要：

- `nsh` **0.3.0**（`nsh --version` 应显示 `0.3.0`）
- rustup 的 stable（`rust-toolchain.toml` 会选它；工程用 Rust edition 2024）
- 系统自带的 clang（Xcode Command Line Tools）
- **不要**设置 `NSH_SOURCE`。依赖走 crates.io 上的 `nsh-core` / `nsh-base` 0.3.0

在仓库根目录：

```sh
nsh check
cargo test
nsh package --target macos
```

`nsh package --target macos` 会在 `.nsh/out/Pulse.app` 写出已 ad-hoc 签名的包，需要 macOS 14 或更新。包内可执行文件是 `Contents/MacOS/Pulse`。开发时用 `nsh run`（在 Mac 上就是 AppKit 窗口，标题是 Pulse）。

## iPhone

在装了 Xcode 的 Mac 上，仓库根目录：

```sh
nsh package --target ios
```

打包后给 Info.plist 补上后台刷新（NSH 0.3.0 的 `nsh.json` 写不了这两项）：

```sh
python3 scripts/ios-background-plist.py .nsh/out/Pulse.app/Info.plist
```

它写入 `UIBackgroundModes` = `fetch` 和 `BGTaskSchedulerPermittedIdentifiers` = `dev.pulse.app.refresh`。没跑这步时，应用会登记失败并打一行 `[pulse] background refresh unavailable`，前台通知照旧。旁加载即使打上了键，iOS 也经常不给后台时间。

不签名时得到 `.nsh/out/Pulse.app`，用 Sideloadly 装到 iPhone。主屏幕名字是 Pulse（`CFBundleDisplayName`）。包里的可执行文件也叫 `Pulse`（`Pulse.app/Pulse`）。带签名时另外得到 `.nsh/out/Pulse.ipa`，里面是 `Payload/Pulse.app`。NSH 0.3.0 的 `nsh.json` 没有部署版本这一项，Info.plist 里的最低系统仍是 iOS 15。Xcode 26 的 iPhoneOS SDK 在没写部署版本时，会把 runner 里的 rquickjs / zstd 编成 SDK 版本（26.5），再按 rustc 的默认 10.0 去链接，于是 `___chkstk_darwin` 找不到。`.cargo/config.toml` 把 `IPHONEOS_DEPLOYMENT_TARGET` 设成 **16.0**，Cargo 编本仓库和它下面生成的 runner 时都会带上。如果打包进程没读到这份配置，在同一条命令前执行 `export IPHONEOS_DEPLOYMENT_TARGET=16.0`。图标那一步的 `actool` 还要本机有 iOS 模拟器运行时（`xcodebuild -downloadPlatform iOS`）。图标是 `assets/icon.png`（1024×1024）。

iPhone（以及窄的 iPad）没有 240 宽的侧栏。顶上是当前平台和刷新，列表单列往下滚，底下是系统标签：动态、看板、设置。登录在设置里。刘海和 Home 条用安全区。外观跟随系统，深色用 `theme.json` 里的暗色。Mac 宽窗口仍是原来的侧栏和两列。

登录层在 iPhone 上铺满屏幕。iOS 的 WKWebView 自带标识里没有 `Safari`，`x.com/login` 会 302 到 `x-safari-https://`，网页视图取消这次导航，页面就是白的（「帧框加载已中断」）。所以 iPhone 上 X 和微博改用一条带 `Safari` 的手机 Safari User-Agent。Instagram 和 Threads 的官方登录页在 iPhone 的内嵌网页里一直是白的（手机版脚本不画表单，桌面版又会把带触摸的 iPhone 当成 iPad）。iPhone 上这两家改成应用内的账号框，由网页存储向站点自己的登录接口提交，`sessionid` 留在 WKWebView 里，再点完成。抓取仍用同一条手机 Safari 标识。Mac 上 Threads 和 Instagram 仍用桌面 Safari 的 User-Agent。Mac 上的微博登录打开 `passport.weibo.com` 的扫码页，同样用桌面 Safari，扫完后回到 `m.weibo.cn` 再用 `api/config` 确认。iPhone 打开手机版 SSO（`entry=wapsso`），因为 `m.weibo.cn/login` 已经是 404。导航和加载失败记在 `[pulse] weibo-login`。新帖通知走 `UNUserNotificationCenter`，图标数字是未读总数。iPhone 登记了 `BGAppRefreshTask`（模式 `fetch`）：系统若叫醒应用，就拉最多 3 条到期的 RSS，再发同样的本地通知和角标。X、Threads、Instagram、微博仍靠前台网页。应用被划掉之后的推送要 APNs，这一版没有推送服务器。Mac 的程序坞和 `osascript` 不会编进 iOS，Mac 也不跑这套后台任务。

RSS 不是侧栏的一列。订阅地址存在 `profile.rss`，和 X / Threads / Instagram / 微博的关注分开；文章的已读标记跟帖子一样写在这张卡片上。默认刷新是 2 分钟，RSS 至少隔 **3 分钟** 再拉一次。网格只挂上屏幕里的卡片（再多一行），所以大约 200 个订阅时，菜单、移除、上移、下移和编辑不会把整库重画一遍。每条订阅留下最近 **24** 篇，正文存到 **2000** 字，打开时会把更长的旧档剪到这个上限。整份配置的 JSON 在保存线程里生成，再写入 `profile.json` 和 `theysay.profile.v1`；点移除不会在界面线程上把几 MB 再序列化一遍。文章正文用共享内存挂在卡片上，保存进行中的修改不会把每篇正文再拷一份。RSS 不用登录。订阅可以放进文件夹：在「管理订阅」里新建、重命名、删除文件夹，勾选后移动。删文件夹不会删订阅，那些账号和订阅回到未分类。侧栏就是这些文件夹，动态和看板都按它筛选，不限平台。管理订阅打开一张列表：可以勾选 RSS 再删除或移动，或一键清掉没有文章的订阅（没拉到内容的也算）。账号格子的菜单也能移到同一个文件夹。文件夹写在 `profile.folders`（`id` 和 `name`）。以前写在 `profile.rss.folders` 的目录，打开时会挪上来，原来的 `folder` id 还对得上。每张卡片的 `folder` 是那个 id，空着就是未分类。OPML 里的目录会变成文件夹，导出时再套回去。OPML 在 RSS 页走系统文件面板：macOS 用 AppKit 的 `NSOpenPanel`（`.opml` / `.xml`）和 `NSSavePanel`（存成 `.opml`）。NSH 0.3.0 没有自带的打开/保存框。iOS 还没接 `UIDocumentPicker`，同一套返回值会说明请改用剪贴板。复制和粘贴仍留在面板下面。

应用 id 是 `dev.pulse.app`。配置写在 `~/Library/Application Support/dev.pulse.app/storage.json`，键仍是 `theysay.profile.v1` 和 `theysay.session.v1`（若改过名，也会读 `pulse.profile.v1` / `pulse.session.v1`）。仪表盘文件在 `~/Library/Application Support/Pulse/profile.json`。

第一次打开时，如果新的存储是空的，而 `~/Library/Application Support/dev.theysay.native/storage.json` 或 `~/Library/Application Support/TheySay/profile.json` 还在，会把关注、已读和设置拷过来。NSH 0.3.0 的网页视图没有单独的 data store 标识，默认仓库跟着 bundle id 走。macOS 上，在第一个网页出现之前，只有当 `~/Library/WebKit/dev.pulse.app`、`~/Library/Caches/dev.pulse.app` 或 `~/Library/HTTPStorages/dev.pulse.app` 这个路径还不存在时，才从旧 id 拷一次。已经存在的目录不会被覆盖。`~/Library/Cookies/*.binarycookies` 不拷：WebKit 会在打开网页时重写那个文件。iPhone 上旧容器不在新应用能读到的地方，换 id 之后要重新登录。日志前缀是 `[pulse]`。微博是否登录只看 `m.weibo.cn/api/config` 的 `data.login`，公开时间线不算已登录。每次打开都会再探一次四个平台的登录。

## 登录

侧栏不再按平台切换。设置里的「账号登录」列出 X、Threads、Instagram、微博的状态（已登录、未登录、还没确认、被拦住）和登录 / 重新登录；RSS 写明不用登录，并打开管理订阅和 OPML。登录层里可以切换这四个页面。窄窗口顶上是文件夹，不是平台。换页会拆掉旧的网页视图再开一个新的，并给它一块有高度的区域，避免 WKWebView 在 0 尺寸时一直空白。Instagram 和 Threads 用桌面 Safari 的 User-Agent，否则它们经常给内嵌网页一个白屏。页眉有「重新加载」，打开超过一会儿还是空的会写出来。Threads 可以用 Instagram 账号。已经停在该平台首页时，提示是「已登录，可以点完成。」点 **完成** 之后，会按顺序用一个屏幕外的网页看三个首页：X 要停在 `/home` 并且有发帖框或账号切换按钮；Threads 要停在 `threads.net` 首页并有发帖框；Instagram 要停在首页并有私信入口或发帖按钮。对上了，设置里那一行就是「已登录」。四个状态分开记住，下次打开不会回到「还没确认」。旧版本只存过一条 X 的状态时，会当成 X 的登录结果。

设置里的「云端同步」可选关闭、iCloud 或自建服务器。iCloud 只有带容器的签名包才写得进去；旁加载的包会说明没有容器，并建议改用自建服务器。自建服务器仍是一份 https 上的订阅列表（文件夹和各平台关注，不含文章和已读），可选口令。模式记在 `profile.sync.mode`（`off` / `icloud` / `https`）。已经填过地址的旧配置会当成自建服务器。

NSH 的网页视图用系统默认的持久化存储（macOS 上是 WKWebView 的默认 data store），所以 cookie 留在这台机器上。读时间线的网页也藏在屏幕外面。同时最多 **2** 个资料页；确认登录时先停掉它们，一次只开一个首页。登录层或原帖弹层打开时同样会先停掉。

## 一个格子怎么读帖

1. 先在侧栏选好平台。空格子上点「添加账号」，输入 `@handle`，或贴这个平台的主页链接，回车。别的平台的链接会被拒绝。已经有账号的格子，点右上角「⋯」可以编辑或移除。添加只会进当前这个平台。
2. 调度器只给当前屏幕附近、到点的格子打开主页（`x.com`、`threads.net/@name`、`instagram.com/name`），同时最多两个。往下滚到的账号才会接着读。
3. 页面加载后注入 `src/extract.js`。脚本按网站读公开 DOM（正文、时间、`/status/`、`/post/`、`/p/` 或 `/reel/` 链接，以及图片），用 `window.nsh.postMessage` 把 JSON 交回来。需要登录时，格子上会写「需要登录 Threads」或「需要登录 Instagram」。
4. `onMessage` 交给 Rust：按 id 合并、按时间从旧到新排、丢掉屏蔽词、标未读。
5. 气泡是原生视图。`$NVDA` 这类 cashtag 变成彩色徽章。链接标题能从 X 卡片上读到就用卡片标题，否则用域名，不再另发请求。照片先按 CDN 地址抓下来（带桌面 Safari 的 User-Agent，以及 `https://x.com/`、`https://www.threads.net/` 或 `https://www.instagram.com/` 的 Referer），再画成圆角图片。原生 `Image` 自己去拉这些地址时不带 Referer，图经常是空的。
6. 点气泡，用弹层里的网页打开原帖；也可以改用系统浏览器。每张卡片里的帖子可以上下滚动；窗口装不下整页格子时，外面那一层也可以滚。窄的单列同样如此。后台刷新不会重挂已经打开的列表，也不会去滚它，除非当时已经贴在底部。NSH 的 `List` 按对象身份比较每一行，不看 key：同一次刷新如果换成新的 JSON 对象，量过的高度会被丢掉，滚动条就跟着跳。帖子没变时沿用原来的对象。未读细条的高度一直留着，数字徽章变了也不会把正文顶开。

脚本**不读** cookie、存储或表单。它大约停 7.5 秒（中间轻轻滚一下，好让时间线多画出几条），然后关掉这个隐藏页，并记下这次的时间。默认 **2 分钟** 后再读，可在设置里改成 30 秒到 30 分钟。

X 只把视口里的帖子放进 DOM，所以一次读到的是第一屏附近的最近帖子，不是全部历史。每个格子最多留下 40 条。

## 会记住什么

本地 JSON（通过 NSH `Storage`）以及设置里的复制 / 粘贴导入，包含：

- 三个平台各自的账号列表，按添加顺序。没有空格子，也没有 1×1 / 2×2 / 3×3
- 正在看的是哪个平台
- 刷新间隔、屏蔽词、浅色 / 深色 / 跟随系统
- 已经读到的帖子和已读标记（换账号再换回来不会丢）

以前只有一份 `cards` 的配置，导入时会按每张卡片当时的平台拆开，空格子丢掉，旧的版面大小忽略。没有写平台的旧卡片仍算 X。导出的 JSON 带上三份列表；那种旧导出还能再导入。上次选中的平台也在这份 JSON 里，下次打开还停在那里。

没有密码，也没有 cookie。NSH 0.3.0 的 Rust API 没有系统保存对话框，所以导出是剪贴板和文本框，不是访达里选一个文件。

## 目录

```text
App.tsx                 窗口：读配置、主题、隐藏网页
src/ui/                 侧栏、格子、气泡、设置、登录、原帖
src/extract.js          注入 X 主页的脚本
src/logic/              纯 Rust：解析、合并、未读、调度、版面
src/bridge.rs           给 TSX 的 JSON 接口（Core）
src/legacy.rs           从旧 bundle id 拷配置和网站数据
src/lib.rs              native module 注册
theme.json              shadcn 风格的 light / dark token
nsh.json                应用名、id、主题
```

以后要加的东西放在这些位置：

- **Discord**：`src/logic/source.rs` 的 `FeedAdapter` 能认出频道链接，但不会加载，界面上也不再预告。X、Threads、Instagram 已经各有一份格子列表。
- **Linux**：同一份 TSX。在装了 GTK 4.14+ 和 WebKitGTK 6.0 的机器上 `nsh run --target linux`。网页视图是 WebKitGTK。
- **iOS**：见下面的「iPhone」。`Core.shellMode` 在手机上返回 `column`。`nsh package --target ios` 要在 Mac 上用 Xcode 打。

## 测试，以及没测到的

在 Linux 上跑过：

- `cargo test`：handle、cashtag、时间、合并与未读、屏蔽词、换账号缓存、导入校验、最多两个隐藏网页、手机用单列
- `nsh check`：crate 的 `cargo check`，以及 TSX 的类型检查

**没有**在 Mac 上打开窗口，所以下面这些都没在真机上看过：

- `nsh run` / `nsh package --target macos` 打出来的 AppKit 窗口
- WKWebView 登录是否真的把 cookie 留给隐藏页
- X 当前的 DOM 是否还长脚本里写的那样（`article[data-testid="tweet"]` 一类）
- 屏幕外的 480×1600 网页是否真能排出帖子（如果祖先把视图裁成 0 像素，X 可能什么都不画）
- 通知。Mac 上已走过 `UNUserNotificationCenter`（日志 `notify path=user-notifications`）。iPhone 用同一套中心发本地通知和图标角标。后台刷新只在系统真的叫醒应用时拉 RSS，旁加载常常不会醒，这一版还没在真机上看过

## 已知限制

- 抓的是主页第一屏附近的帖子，不是全部历史。
- 网站改版、未登录、人机验证或频率限制时，格子上会写原因，不会有数据。
- 链接标题只来自页面里已经画出来的卡片，或 URL 的域名。
- 照片只留 CDN 地址。显示时再下载，不写进配置。太大或下不下来的图会空着一块灰色。
- 会员专属帖、拖拽改格子大小，这一版都没有。Mac 的新帖通知和程序坞数字在应用开着时更新。iPhone 的后台刷新只拉 RSS，而且要系统批准；划掉应用之后的推送还没有 APNs。
- 帖子正文可以选择、复制（`Text selectable`）。点「打开」仍进入原文。
- 翻译和早报走设置里的同一套 OpenAI 兼容接口（地址、API Key、模型）。没有内置密钥。早报用昨天的星标写成中文口播稿。设置里的「本机生成早报」关掉后，这台设备到点不会自己写。Mac 和 iOS 用系统的 `AVSpeechSynthesizer` 播放。星标写在本地 `profile.json`，订阅同步不带帖子。
