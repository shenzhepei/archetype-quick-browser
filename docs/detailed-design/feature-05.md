# Feature 05 Chromium 内核迁移详细设计

| 字段 | 内容 |
|------|------|
| Feature ID | `feature-05` |
| 状态 | 实施中 |
| 对应 PRD | [feature-05.md](../prd/feature-05.md) |
| 主要模块 | 新增 `arch-chromium`、`arch-browser::ui`、打包脚本；legacy `archetype-runtime` |

## 1. 技术选型

- 内核：Chromium Embedded Framework，通过 `tauri-apps/cef-rs` 的 `cef` crate 固定版本接入。
- 窗口模式：macOS 使用 CEF windowed child browser；父句柄来自 GPUI `Window` 的 `raw-window-handle` AppKit `NSView`。
- Browser Chrome：继续使用 GPUI，不迁移到 Chromium HTML UI。
- 不选 `wry`，因为 macOS 后端是 WebKit；不选 CDP 外部进程，因为无法形成可靠嵌入、输入、弹窗和发布边界。

## 2. 进程结构

```text
Archetype.app (GPUI Browser process)
  |- arch-chromium host
  |- CEF Browser instances
  |- Archetype Helper.app --type=renderer
  |- Archetype Helper (GPU).app --type=gpu-process
  |- Archetype Helper (Plugin).app
  `- Archetype Helper (Renderer).app
```

Browser 进程拥有标签、地址栏、历史和权限决策。CEF Renderer 执行网站 JavaScript/DOM/CSS；GPU/Utility 使用 CEF 默认隔离。`archetype-runtime` 不再是普通网页 Renderer，迁移期只保留 legacy 测试。

## 3. `arch-chromium` 边界

新增 crate 对 Browser 暴露稳定接口，不把 CEF 类型扩散到 UI：

```text
ChromiumRuntime::initialize(RuntimeConfig)
ChromiumRuntime::attach(parent_view, bounds)
ChromiumRuntime::create_tab(TabId, url)
ChromiumRuntime::show_tab / hide_tab / close_tab
ChromiumRuntime::navigate / back / forward / reload / stop
ChromiumRuntime::resize(bounds, scale_factor)
ChromiumEvent::{Loading, UrlChanged, TitleChanged, FaviconChanged,
                NavigationFailed, RendererTerminated, PopupRequested}
```

CEF callback 只发送有界事件到 Browser 主线程，不直接修改 GPUI Entity 或 Store。导航带 Tab ID 与 generation，关闭或被新导航替代的 callback 被丢弃。

## 4. 初始化与消息循环

macOS 主程序先定位 bundle 内 `Chromium Embedded Framework.framework`，加载 library，构造 `cef::Args`，对子进程调用 `cef_execute_process`，Browser 进程再调用 `cef_initialize`。设置 `multi_threaded_message_loop = 0`，由 GPUI 主线程定时调用 `cef_do_message_loop_work`，避免两个 AppKit 主循环竞争。

CEF 不存在或程序不是合法 bundle 时返回 `ChromiumUnavailable`。开发运行使用仓库脚本构建临时 `.app` 后启动，不把裸 `cargo run` 作为 CEF 正常入口。

## 5. 视图嵌入与标签

内容区域每次 layout 后把物理像素 bounds 传给 `arch-chromium`。CEF child view 使用 GPUI 根 `NSView` 为 parent，坐标从 GPUI 顶左系转换为 AppKit 底左系。只有选中 Browser 可见并接收焦点；后台 Browser 保持页面、JS heap、网络和表单状态。

初始常驻上限沿用 8 个标签，但 CEF 标签不使用旧 display-list hibernation。达到上限时先冻结后台页，再按内存压力丢弃并使用 Chromium session history 恢复。

## 6. 导航与状态同步

- 地址栏提交调用 CEF MainFrame `load_url`。
- back/forward/reload/stop 调用 CEF Browser/Host API。
- DisplayHandler 回写标题、地址和 favicon；LoadHandler 回写 loading 状态和错误。
- 只有 committed main-frame navigation 写 Archetype 历史；subframe、资源请求、失败和内部页不写。
- popup 默认转换为新 Archetype 标签，禁止创建脱离 Browser Chrome 的未管理窗口。

## 7. Profile、Cookie 与权限

CEF RequestContext 使用持久 cache path `<profile>/chromium/Default`。Cookie、cache、localStorage、IndexedDB、Service Worker 和网络栈全部由 Chromium 管理，不与旧 `CookieJar` 双写。Archetype SQLite 继续保存 Spaces、tabs、书签、历史和设置。

权限请求通过 CEF PermissionHandler 转交 Browser UI。默认拒绝摄像头、麦克风、地理位置、通知和剪贴板写入，用户决定后按 origin 持久化。证书错误默认拒绝，不能沿用开发绕过。

## 8. 内部页面

`archetype://` 不注册为网站可自由访问的 CEF scheme。选中内部标签时隐藏 CEF child view并渲染现有 GPUI 页面。网站发起 `archetype:` 导航或 popup 时由 RequestHandler 拒绝。若未来设置页改用 Chromium WebUI，使用单独不可网络访问的 scheme factory 和显式 IPC 白名单。

## 9. 打包

使用 `cef-rs` bundler 生成 macOS 主 app 和 Helper app，复制 Framework、Resources 与 Locales。构建产物顺序：Rust binaries → bundle → Helper 签名 → Framework 签名 → 主 app 签名 → notarization。CI 缓存按 CEF version/target/hash 分区，不把数百 MB Framework 提交到 Git。

## 10. 迁移步骤

| 阶段 | 内容 | 状态 |
|------|------|------|
| C1 | 固定 CEF 版本、runtime locator、bundle metadata、Helper 入口 | 已实现；当前网络无法下载 Framework，真实链接待镜像/`CEF_PATH` 验证 |
| C2 | 单 CEF child view 嵌入 GPUI 内容区 | 待实施 |
| C3 | 多标签、导航 callback、loading/title/favicon/history | 待实施 |
| C4 | RequestContext、Cookie/站点数据、权限和内部协议隔离 | 待实施 |
| C5 | Helper 沙箱、签名、崩溃恢复和发布验证 | 待实施 |
| C6 | 默认关闭 legacy renderer并清理支持矩阵 | 待实施 |

## 11. 测试

- 单元测试：配置路径、Tab/generation 路由、内部 scheme 拒绝、事件去重。
- 集成测试：CEF 启动、JS DOM mutation、CSS 综合页、导航/刷新/停止、两标签状态保持。
- profile 测试：Cookie/localStorage 重启恢复与 profile 隔离。
- 打包测试：Framework/Resources/Helpers 完整性、codesign verify 和干净环境启动。
- 性能测试：标签切换延迟、首屏时间、后台 CPU、Renderer 崩溃后 Browser 存活。

真实 CEF 链接由 `chromium-runtime` Cargo feature 启用。普通 workspace 测试不下载数百 MB Framework；发布构建必须启用该 feature，且缺失 CEF 时失败，不允许生成不含 Chromium 的发布包。CEF 下载源可通过 `CEF_DOWNLOAD_URL` 指向受控镜像，或通过 `CEF_PATH` 使用预置且经过 hash 校验的分发包。
