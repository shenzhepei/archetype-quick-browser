# Feature 07 网页右键菜单详细设计

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-07` |
| 状态 | 已实现 |
| 对应 PRD | [feature-07.md](../prd/feature-07.md) |
| 主要模块 | `src/main/controller.ts`、`src/main/page-context-menu.ts` |

## 1. 事件与菜单归属

`BrowserController.bindTab` 为每个普通网页 `WebContentsView.webContents` 监听 Chromium `context-menu` 事件。事件闭包保留目标 `BrowserTab` 与 `ContextMenuParams`，所有命令直接操作该目标，避免菜单打开后标签状态变化导致误操作。

菜单由主进程通过 Electron `Menu` 构建并挂到 `BaseWindow`，无需 Renderer IPC 或网页脚本参与，因此显示层级高于网页内容。

## 2. 命令设计

- 后退、前进读取目标 `webContents.navigationHistory`，无历史时保持菜单可见但禁用。
- 重新加载调用目标 `webContents.reload()`。
- 网页另存为调用系统 `dialog.showSaveDialog`；确认路径后使用 `webContents.savePage(path, 'HTMLComplete')` 保存 HTML 和关联资源目录。
- 查看网页源代码为目标 HTTP/HTTPS URL 创建 `view-source:` 新标签；其他 scheme 禁用该命令。
- 检查调用目标 `webContents.inspectElement(params.x, params.y)`，由 Chromium 打开 DevTools 并定位元素。

## 3. 保存策略

默认目录使用系统 Downloads，文件名优先取页面标题，标题为空时取 hostname，最终回退到 `page.html`。文件名移除 Windows、macOS 和 Linux 常见非法字符并限制长度，统一补充 `.html` 后缀。

用户取消对话框时不执行任何操作。`savePage` 失败时使用当前浏览器语言显示原生错误对话框，不向网页暴露本地路径。

## 4. 测试

- 单元测试菜单顺序、国际化文案和后退/前进可用状态。
- 单元测试建议文件名清理、后缀和回退规则。
- `pnpm test`、`pnpm typecheck`、`pnpm build` 通过。
- 真实 Electron 页面验证菜单显示、保存 HTML、源代码标签和 DevTools 检查元素。
