# Feature 08 主菜单新建与扩展管理详细设计

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-08` |
| 状态 | 已实现 |
| 对应 PRD | [feature-08.md](../prd/feature-08.md) |
| 主要模块 | `src/main/index.ts`、`src/main/controller.ts`、`src/main/extension-service.ts`、`src/renderer/browser/InternalPage.tsx` |

## 1. 多窗口路由

主进程维护以 shell `webContents.id` 为键的窗口上下文，保存对应 `BaseWindow`、shell `WebContentsView` 和 `BrowserController`。所有浏览器 IPC 根据 `event.sender.id` 查找上下文，不再依赖单一全局窗口。

首个窗口恢复持久化标签；“新窗口”创建空白标签并关闭标签会话持久化，避免新窗口覆盖主窗口下次启动的标签集合。书签、历史和设置仍写入共享 `BrowserStore`。

## 2. 主菜单

主菜单顺序为：打开新的标签页、打开新的窗口、分隔线、历史记录、扩展程序子菜单、设置。菜单在触发窗口上弹出，所有回调闭包捕获该窗口上下文。

扩展程序子菜单只接受固定的“管理扩展程序”命令，打开或定位 `archetype://extensions`。Renderer 不能发送任意菜单模板或内部 URL。

## 3. 扩展服务

`ExtensionService` 绑定 `persist:archetype` session，并使用 Electron `session.extensions` API：

- 启动时读取持久化目录并逐个调用 `loadExtension`，单个目录失败不会终止启动。
- 列表输出最小摘要：ID、名称、版本、描述和目录。
- 加载时由主进程打开系统目录选择器，Renderer 不传入路径；成功后去重保存目录。
- 移除时只接受当前 session 中存在的扩展 ID，调用 `removeExtension` 并删除对应持久化目录。

## 4. 管理页面

内部页展示标题、加载已解压扩展命令、空状态以及扩展列表。列表项显示名称、版本、描述和目录，并提供移除按钮。加载或移除完成后重新读取列表；错误通过受控 IPC 消息返回并在页面内展示。

## 5. 验证

- 单元测试内部页标题、扩展管理渲染和桥接调用。
- 单元测试扩展摘要与持久化目录去重。
- `pnpm test`、`pnpm typecheck`、`pnpm build` 通过。
- Electron 实机验证主菜单层级、新标签、新窗口和扩展管理标签定位。
