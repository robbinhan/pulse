# Pulse（脉搏）

Pulse 是用 [NSH 0.3.0](https://github.com/longbridge/nsh/releases/tag/v0.3.0) 写的 Mac / iOS 原生聚合阅读器。它把 **X**、**Threads**、**Instagram**、**微博** 和 **RSS** 收成一条按时间排列的 **动态**，新的在上。这些来源共用文件夹：全部、未分类，以及自己建的目录。登录在设置里。默认首页是动态；**看板** 是账号格子，用来加人和管理订阅。窄窗口和 iPhone 是单列，底下是动态、看板、设置。窗口标题和主屏幕名字是 Pulse，应用里写的是脉搏。

界面是 TSX。时间线合并、未读、星标、屏蔽词、翻译和调度在 Rust 里。

## 功能

- **未读。** 新帖标未读。Mac 的程序坞数字和 iPhone 的图标角标都是未读总数。已读写在本机卡片上。
- **星标。** 星标留在本地配置里。云端同步不带走帖子。
- **翻译。** 在设置里填写 OpenAI 兼容接口的地址、API Key 和模型。这三项的默认值是空的，仓库里也不写密钥。请求发到 `{地址}/chat/completions`。模型留空时用 `gpt-4o-mini`。语言可以是简体中文、English、日本語，或跟随系统。译文按帖子记下，再次打开不会重发。
- **早报。** 用昨天的星标写成中文口播稿，走同一套翻译接口。「本机生成早报」默认开着，本地时间默认早上 8 点。关掉之后，这台设备到点不会自己写；手动「生成早报」仍然可用。没填接口，或昨天没有星标，就跳过。Mac 和 iOS 用系统的 `AVSpeechSynthesizer` 播放。
- **云端同步。** 可选关闭、iCloud 或自建 https。同步的是文件夹和各平台关注。文章、已读、星标和 API Key 留在本机。iCloud 要带容器的签名包；旁加载的包会说明没有容器，并建议改用自建服务器。自建服务器可以另设口令。
- **OPML。** 导入和导出 RSS。OPML 里的目录会变成文件夹，导出时再套回去。Mac 用系统打开 / 保存面板（`.opml` / `.xml`）。iPhone 用剪贴板。
- **复制。** 帖子正文可以选择、复制。点「打开」进入原文，也可以改用系统浏览器。
- **刷新和屏蔽。** 默认 2 分钟，可改成 30 秒到 30 分钟。RSS 至少隔 3 分钟再拉一次。屏蔽词在设置里。每条账号最多留 40 条帖子；每条 RSS 留最近 24 篇，正文存到 2000 字。同时最多打开 2 个隐藏资料页。

账号在看板上添加：输入 `@handle`，或贴该平台的主页链接。订阅可以放进同一个文件夹。删文件夹只把里面的账号和订阅送回未分类。

## 构建和运行

在 Apple 芯片的 Mac 上，于仓库根目录操作。

需要：

- `nsh` **0.3.0**（`nsh --version` 应显示 `0.3.0`）
- rustup 的 stable（`rust-toolchain.toml` 会选它；工程用 Rust edition 2024）
- 系统自带的 clang（Xcode Command Line Tools）

依赖走 crates.io 上的 `nsh-core` / `nsh-base` 0.3.0。让 `NSH_SOURCE` 保持未设置。

### 检查

```sh
nsh check
cargo test
```

`nsh check` 会做 crate 的 `cargo check`，以及 TSX 的类型检查。开发时用 `nsh run`。在 Mac 上这是一个 AppKit 窗口，标题是 Pulse。

### 打包 macOS

```sh
nsh package --target macos
```

产物是 `.nsh/out/Pulse.app`，已经 ad-hoc 签名，需要 macOS 14 或更新。包内可执行文件是 `Contents/MacOS/Pulse`。

### 打包 iPhone

在装了 Xcode 的 Mac 上：

```sh
nsh package --target ios
python3 scripts/ios-background-plist.py .nsh/out/Pulse.app/Info.plist
```

第二条给 Info.plist 补上后台刷新：`UIBackgroundModes` = `fetch`，`BGTaskSchedulerPermittedIdentifiers` = `dev.pulse.app.refresh`。NSH 0.3.0 的 `nsh.json` 写不了这两项。没跑这步时，应用会登记失败并打一行 `[pulse] background refresh unavailable`，前台通知照旧。

未签名时得到 `.nsh/out/Pulse.app`，用 Sideloadly 装到 iPhone。主屏幕名字是 Pulse，包内可执行文件也叫 `Pulse`。带签名时另外得到 `.nsh/out/Pulse.ipa`，里面是 `Payload/Pulse.app`。

`.cargo/config.toml` 把 `IPHONEOS_DEPLOYMENT_TARGET` 设成 **16.0**。Xcode 26 的 iPhoneOS SDK 在没写部署版本时，会把 runner 里的依赖编成 SDK 版本，再按过低的目标去链接，于是 `___chkstk_darwin` 找不到。打包进程若没读到这份配置，在同一条命令前执行：

```sh
export IPHONEOS_DEPLOYMENT_TARGET=16.0
```

图标是 `assets/icon.png`（1024×1024）。图标那一步的 `actool` 需要本机已有 iOS 模拟器运行时（`xcodebuild -downloadPlatform iOS`）。

Linux 上同一份界面可以用 `nsh run --target linux`（GTK 4.14+ 和 WebKitGTK 6.0）。打包 Mac 和 iOS 仍要在 Mac 上做。

## 配置在哪

应用 id 是 `dev.pulse.app`。

| 内容 | 路径 |
| --- | --- |
| NSH Storage | `~/Library/Application Support/dev.pulse.app/storage.json` |
| 整份配置 | `~/Library/Application Support/Pulse/profile.json` |
| 网站数据 | `~/Library/WebKit/dev.pulse.app`、`~/Library/Caches/dev.pulse.app`、`~/Library/HTTPStorages/dev.pulse.app` |

Pulse 把配置和登录状态写在 NSH Storage 的 `~/Library/Application Support/dev.pulse.app/storage.json` 里。同一份文件里如果已有 `pulse.profile.v1` / `pulse.session.v1`，打开时也会读。Cookie 留在这个 bundle id 的网页存储里，不写进配置 JSON。日志前缀是 `[pulse]`。

设置里的整份配置可以复制到剪贴板，或从文本框粘贴回来。存成文件时用 `pulse-profile.json`。iPhone 上同一份数据在本应用容器里。

## 目录

```text
App.tsx            窗口：读配置、主题、隐藏网页
src/ui/            动态、看板、设置、登录、翻译、早报、同步
src/extract.js     注入各站主页的脚本
src/logic/         解析、合并、未读、RSS、翻译、早报、调度
src/bridge.rs      给 TSX 的 JSON 接口（Core）
src/store.rs       读写本地配置
src/legacy.rs      启动时接上本机已有的配置和网站数据
src/notify.rs      本地通知（Mac / iOS）
src/speech.rs      早报朗读
nsh.json           应用名 Pulse、id dev.pulse.app、主题
theme.json         浅色 / 深色
assets/icon.png    1024×1024 图标
scripts/           iOS 后台刷新的 Info.plist 补丁
```

## 测试

`cargo test` 覆盖 handle、时间、合并与未读、屏蔽词、RSS / OPML、翻译地址、早报、同步合并，以及同时最多两个隐藏网页。`nsh check` 做编译和 TSX 类型检查。界面旁路测试在 `App.test.tsx`、`SettingsLogin.test.tsx`、`WeiboAdd.test.tsx` 和 `src/ui/*.test.tsx`。

这些测试不打开 Mac 或 iPhone 窗口。打包出来的界面、站点当前的 DOM，以及系统是否真的给后台时间，要以真机为准。

## 已知限制

- 读到的是各站主页第一屏附近的帖子，不是全部历史。网站改版、未登录、人机验证或频率限制时，格子上会写原因。
- 链接标题来自页面上已经画出的卡片，否则用域名。图片只存 CDN 地址，显示时再下载；下不来就留一块灰色。
- 会员专属帖和拖拽改格子大小，这一版没有。
- iPhone 的后台刷新只拉最多 3 条到期的 RSS，而且要系统批准。旁加载经常不会被叫醒。划掉应用之后的推送要 APNs，这一版没有推送服务器。X、Threads、Instagram 和微博靠前台网页。
- iPhone 上 Instagram 和 Threads 的官方登录页在内嵌网页里是白的，这两家改用应用内账号框，会话留在网页存储里。X 和微博使用带 Safari 的手机标识。
- iPhone 的 OPML 走剪贴板。
- 翻译和早报都要自己在设置里填接口。本机 Apple 翻译要 iOS 18 或 macOS 15 的 Swift 接口，这一版走上面的 HTTP 接口。
