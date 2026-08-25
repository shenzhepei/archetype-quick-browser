# Feature 02 网页上层主菜单详细设计

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-02` |
| 状态 | 已实现 |
| 对应 PRD | [feature-02.md](../prd/feature-02.md) |
| 主要模块 | `src/main/index.ts`、`src/preload/index.ts`、`src/renderer/browser/Toolbar.tsx` |

## 1. 根因与方案

普通网页的 `WebContentsView` 位于 Browser Chrome view 上方，仅通过 bounds 避开标签栏和工具栏。React 菜单虽然具有较高 CSS `z-index`，但伸入网页 bounds 的像素仍由上层原生 view 覆盖。

三点按钮改为调用白名单 IPC `browser:show-menu`。主进程使用 Electron `Menu.buildFromTemplate` 创建固定菜单，并通过 `Menu.popup` 绑定主窗口。系统菜单处于所有窗口内容 view 之上。

## 2. 数据与命令

Renderer 只发送按钮左下角的窗口内坐标 `{ x, y }`。主进程验证坐标为有限数值并限制在窗口内容 bounds 内，不接受标签、角色、回调或任意模板。

主进程根据当前 `BrowserSettings.language` 生成以下固定菜单：

- 历史记录：导航到 `archetype://history`。
- 设置：导航到 `archetype://settings/appearance`。

语言不与设置平级。`archetype://settings/languages` 由 React 内部页渲染，设置左栏提供入口，内容区通过 English/简体中文分段控件更新 `BrowserSettings.language`。

## 3. 工具标签复用

头像和原生菜单不调用当前标签的 `navigate`，而是调用 `BrowserController.openUtilityPage`：

- 设置以 `archetype://settings/` 为单例分组；任意设置栏目已打开时直接选择该标签并保留当前栏目。
- 历史以 `archetype://history` 为单例；已打开时直接选择。
- 未找到对应标签时创建新的内部页标签，原网页标签及其 Chromium navigation history 保持不变。

设置页左侧栏目仍调用 `openInternal`，因此只更新当前设置标签，不创建额外标签。

## 4. Renderer 调整

`Toolbar` 不再维护或渲染 HTML 浮层；三点按钮点击时读取自身 `getBoundingClientRect()` 并调用 bridge。`BrowserShell` 删除菜单开关状态，旧 `BrowserMenu` 组件及专用 SCSS 样式移除。静态 Web preview 的 demo bridge 将该命令实现为空操作。

Browser Chrome 为 `body`、表单控件、标签标题和设置标题指定固定像素行高。中英文字体 fallback 只改变字形，不改变 CSS line box 高度。

`internalPageTitle(url, language)` 统一生成历史和设置栏目标题。创建/恢复内部标签、设置侧栏导航及语言变更都调用同一规则；语言变更会重新计算所有内部标签标题。标题格式为 `Settings - Appearance`、`Settings - Language`、`Settings - About Archetype` 及对应中文。

设置左侧栏目与内容区标题复用同一个 Lucide 图标；外观使用 `Palette`，语言使用 `Languages`，避免同一栏目出现不同视觉语义。

原生菜单项使用 Lucide History 与 Sliders Horizontal 路径生成 `NativeImage`，macOS 标记为 template image 以跟随系统菜单主题。菜单文本追加固定 em-space 预留，保证中英文状态均具有更宽的稳定菜单宽度。

## 5. 验证

- 普通网站加载完成后，菜单完整覆盖网页内容。
- 原生菜单只显示历史与设置；设置语言页可切换 English 和简体中文。
- 打开设置或历史不替换当前网页，重复打开定位到现有工具标签。
- 中英文往返切换时标签栏、工具栏和设置布局无纵向位移。
- 设置栏目及语言切换后标签标题准确更新；原生菜单宽度和图标可见。
- `pnpm typecheck`、`pnpm test`、`pnpm build` 通过。
