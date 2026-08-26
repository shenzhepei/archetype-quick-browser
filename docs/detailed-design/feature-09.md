# Feature 09 网页打印入口详细设计

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-09` |
| 状态 | 已实现 |
| 对应 PRD | [feature-09.md](../prd/feature-09.md) |
| 主要模块 | `src/main/controller.ts`、`src/main/index.ts`、`src/main/page-context-menu.ts` |

## 1. 打印能力

`BrowserController` 提供活动网页可打印判断和打印命令。可打印 URL 限定为 HTTP、HTTPS、file 和 view-source 页面；`about:blank` 与 `archetype://` 页面不进入打印流程。

打印调用目标 `webContents.print({ printBackground: true })`，不启用 silent。Chromium 负责展示操作系统打印界面；用户取消时不提示错误，没有打印机或其他回调失败时通过所属窗口显示本地化原生错误对话框。

## 2. 菜单接入

- 网页 `context-menu` 的闭包持有目标 `BrowserTab`，在“网页另存为”后插入“打印”，直接打印触发右键的网页。
- 主菜单捕获当前 `WindowContext`，在“历史记录”后插入“打印”，可用状态来自该窗口 Controller 的活动页。

两个入口都由主进程构建固定原生菜单，不增加 Renderer 文件系统或打印机权限。

## 3. 验证

- 单元测试网页右键菜单打印项顺序、文案和回调。
- `pnpm test`、`pnpm typecheck`、`pnpm build` 通过。
- Electron 实机验证两处入口均打开系统打印对话框，并取消测试打印任务。
