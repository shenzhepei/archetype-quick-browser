# Feature 12 标签与书签栏溢出管理详细设计

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-12` |
| 状态 | 已实现 |
| 对应 PRD | [feature-12.md](../prd/feature-12.md) |
| 主要模块 | `src/renderer/browser/TabStrip.tsx`、`src/renderer/browser/BookmarksBar.tsx`、`src/main/index.ts`、`src/main/controller.ts`、`src/shared/browser.ts` |

## 1. 标签布局

标签区域由可压缩的标签列表和固定宽度新增按钮组成。移除横向 `auto` 滚动，标签使用相同伸缩参数从 220px 等宽收窄至 30px。标签自身启用 inline-size 容器查询，宽度低于阈值后隐藏标题并居中显示 favicon；活动标签始终以关闭按钮替换 favicon，其他紧凑标签悬停时进行同样替换，确保极窄状态仍可关闭。

## 2. 书签可见性测量

`BookmarksBar` 使用 `ResizeObserver` 监听栏宽。每个书签按钮保留可测量引用；溢出项移出正常布局但仍可测量。算法先求全部按钮宽度与间距，全部可容纳时隐藏“更多”，否则预留 32px 按钮宽度并按顺序计算完整可见项数量。

书签数量、标题或容器宽度变化都会重新计算。双右箭头按钮只在有溢出项时显示，并把溢出书签 ID 与按钮底部坐标交给固定 IPC。

## 3. 原生菜单

书签栏右键 IPC 只传弹出坐标。主进程从当前窗口 Controller 读取活动标签和书签状态，构造“添加网页”“添加文件夹”“打开书签管理器”固定菜单。添加网页调用仅新增语义的方法，绝不因重复操作取消已有书签。

溢出菜单 IPC 只接受坐标和书签 ID 数组。主进程按当前状态过滤 ID，生成带持久化 favicon 的菜单项，点击后在当前标签导航。

## 4. 新建文件夹入口

Controller 新增打开书签管理器并请求新建文件夹的方法。已有管理器标签时定位并切换其内部 URL；没有时创建。`InternalPage` 识别该 URL 后展开根目录新建文件夹表单。

## 5. 验证

- 组件测试标签结构不再使用滚动容器宽度撑开逻辑。
- 组件测试书签右键 IPC、“更多”按钮和溢出 ID。
- 主进程类型检查、`pnpm test` 与 `pnpm build` 通过。
- Electron 实机检查大量标签只显示 favicon、无系统箭头，书签“更多”和右键菜单不被网页覆盖。
