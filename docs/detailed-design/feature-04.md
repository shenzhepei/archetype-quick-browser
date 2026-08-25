# Feature 04 稳定标签宽度与标签菜单详细设计

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-04` |
| 状态 | 已实现 |
| 对应 PRD | [feature-04.md](../prd/feature-04.md) |
| 主要模块 | `src/renderer/browser/TabStrip.tsx`、`src/renderer/styles/main.scss`、`src/main/controller.ts`、`src/main/index.ts` |

## 1. 标签宽度

`TabStrip` 根据标签数量设置容器 flex basis：`标签数 × 220px + 新建按钮占位`。平台样式继续通过 max-width 扣除窗口控制安全区。

每个标签使用相同的 `220px` width 与 flex basis、`116px` 最小宽度。容器受可用宽度约束时所有标签以相同参数等比例收缩，宽度只依赖窗口和标签数量，不依赖文本 intrinsic size。达到最小宽度后 `.tabs-scroll` 保持横向滚动。

## 2. 上下文菜单 IPC

右键事件发送 `{ tabId, x, y }`。主进程校验坐标为有限数值并限制在窗口 bounds，校验标签 ID 当前存在，再构建固定 Electron `Menu`：

- Reload：调用目标普通网页 `webContents.reload()`；内部页重新发布状态。
- Close：关闭目标标签，沿用相邻标签选择规则。
- Close other tabs：保留目标标签并选择它。
- Close tabs to the right：按当前标签顺序关闭目标右侧标签；活动标签被关闭时选择目标标签。

主进程根据 Browser Settings 语言生成菜单文案。菜单项可用状态从当前标签顺序计算，Renderer 无法传入标签、命令或回调。

## 3. 验证

- 组件测试标题变化前后宽度公式输入不变，并验证右键 IPC 参数。
- Controller 行为通过真实 Electron 多标签操作验证。
- `pnpm test`、`pnpm typecheck`、`pnpm build` 通过。
