# 01 Archetype Chromium 浏览器详细设计

| 字段 | 内容 |
| --- | --- |
| 规范号 | `01` |
| 状态 | 实施中 |
| 对应 PRD | [01-Archetype-Chromium浏览器-PRD.md](../prd/01-Archetype-Chromium浏览器-PRD.md) |
| 主要模块 | `src/main`、`src/preload`、`src/renderer`、`src/shared` |

## 1. 技术架构

Electron 主进程拥有窗口、Chromium 页面实例、session、导航和持久化。React Renderer 是受限 Browser Chrome，仅绘制标签栏、工具栏、菜单和内部页。普通网站不加载进 React DOM，而是由窗口内容区中的 `WebContentsView` 直接渲染。

```text
BaseWindow
  |- React Browser Chrome WebContentsView (sandboxed renderer + preload whitelist)
  `- active WebContentsView (Chromium website)
       `- persist:archetype session
```

## 2. 标签与视图

`BrowserController` 维护 `Map<TabId, BrowserTab>`。每个 `BrowserTab` 含一个独立 `WebContentsView` 和可序列化 `TabState`。`BaseWindow` 先加入 Browser Chrome view，再把活动网页 view 加到内容区上层。切换时从 `contentView` 移除旧网页视图并加入新视图，实例和页面状态不销毁；关闭时调用 `webContents.close()`。

React 通过 `ResizeObserver` 上报内容区 bounds，主进程只调整当前普通网页视图。`archetype://` 标签不加载到 Chromium，当前视图被移除，由 React 渲染对应内部页。

## 3. 导航与同步

- 地址输入先归一化：完整 scheme 原样保留，域名补 `https://`，localhost/IP 补 `http://`，其余转 Google 搜索。
- `did-start-loading`、`did-stop-loading`、`did-navigate`、`page-title-updated` 和 `page-favicon-updated` 更新标签状态。
- `navigationHistory` 提供前进/后退能力；弹窗通过 `setWindowOpenHandler` 转成新标签。
- 主进程通过 `browser:state` 单向发布完整快照；Renderer 通过 preload 暴露的命令 API 发起操作。

## 4. 持久化

产品元数据保存在 Electron `userData/browser-state.json`，临时文件写完后 rename，内容包括标签 URL、选择位置、收藏、历史与偏好。历史最多保留 1000 条。

网站数据使用 `persist:archetype` partition，由 Chromium 管理 Cookie、HTTP cache、localStorage、IndexedDB 和 Service Worker。两类数据不双写。读取损坏 JSON 时恢复默认 `about:blank` 标签。

## 5. 安全边界

- Browser Chrome 和网站均启用 sandbox；Browser Chrome 额外启用 context isolation，禁用 Node integration。
- preload 仅暴露 `ArchetypeBridge`；网站 WebContents 不安装 preload。
- 第一版 `setPermissionRequestHandler` 默认拒绝所有权限。
- `archetype://` 仅作为 Browser Controller 状态，不注册为网站可访问 scheme。
- 后续必须增加下载确认、证书错误 UI、按 origin 权限管理和导航安全测试。

## 6. UI 与国际化

标签栏和工具栏使用稳定高度，新增标签按钮位于最后一个标签之后。加载时 spinner 包围缩小的 favicon，加载完成后恢复普通 favicon；缺失时使用默认 Globe 图标。地址栏尾部是收藏按钮，右侧依次是头像设置入口和三点菜单。

主题支持 system/light/dark。i18next 管理等价的英文与简体中文资源，缺失或无效语言值回退英文，切换同步更新 `html.lang` 与 localStorage。

## 7. 构建与验证

- `electron-vite` 同时构建 main、preload 与 renderer。
- Electron Builder 生成 macOS DMG/ZIP。
- Vitest + Testing Library 验证语言切换、标签操作、设置和历史空状态，并输出 LCOV。
- CI 在 macOS Node 24 执行类型检查、测试与完整构建；Coverage 工作流通过 OIDC 上传 Codecov。
- Pages 只部署 Browser Chrome 的静态交互预览，不具备 Electron `WebContentsView`，不能替代桌面应用验收。

## 8. 当前状态

已实现核心进程边界、多标签、导航、状态 callback、favicon/loading、收藏、历史、主题、语言、元数据与 Chromium profile 持久化。待验证真实 Electron 输入/缩放/弹窗行为，并待实现权限 UI、下载管理、崩溃恢复、签名、公证和自动更新。
