# Feature 10 Chrome 风格书签与历史菜单详细设计

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-10` |
| 状态 | 已实现 |
| 对应 PRD | [feature-10.md](../prd/feature-10.md) |
| 主要模块 | `src/main/index.ts`、`src/main/controller.ts`、`src/renderer/browser/InternalPage.tsx` |

## 1. 主菜单数据

主进程从触发窗口 `BrowserController.state()` 读取历史和书签，不接受 Renderer 提供的列表。历史使用现有倒序访问顺序取前 8 条；书签按 `createdAt` 倒序取前 8 条。

原生菜单标题压缩为单行有限长度，完整 URL 放在 tooltip；点击最近项调用目标窗口 `createTab(url)`。空列表使用禁用菜单项保持二级菜单结构稳定。

## 2. 历史记录二级菜单

历史子菜单首先提供“显示完整历史记录”，打开或定位 `archetype://history`；有最近记录时添加分隔线和最多 8 条记录，否则显示“没有最近历史记录”。

## 3. 书签二级菜单

书签子菜单顺序为：收藏/取消收藏当前页、显示所有书签、分隔线、最近书签。内部页和 `about:blank` 禁用当前页收藏命令。最近书签为空时显示禁用空状态。

“显示所有书签”打开或定位 `archetype://bookmarks`。`BrowserController.removeBookmark(id)` 只移除当前状态中存在的书签 ID，并持久化、发布最新状态。

新增书签先复制活动标签当前 favicon URL，随后通过该标签所属 Chromium session 获取图标。只接受 `image/*` 且不超过 1 MiB 的响应，转换为 data URL 后更新书签并持久化。获取失败时保留原 URL，不影响收藏操作。

对于 favicon 字段加入前创建的旧书签，已收藏 URL 再次触发 `page-favicon-updated` 时自动回填并执行同一缓存流程。

## 4. 书签管理页

书签页复用内部页布局，以列表形式展示创建时间、标题和 URL。点击内容区域导航到书签 URL；独立移除按钮调用白名单 IPC，避免触发导航。空列表显示明确空状态。

书签栏和管理页使用共享 `BookmarkFavicon` 组件，图片加载失败时回退到默认地球图标。原生书签二级菜单从已缓存的 data URL 创建 `NativeImage`；旧书签或尚未缓存的远程 URL 保持无原生图标，不阻塞菜单弹出。

## 5. 验证

- 单元测试内部页标题、书签列表打开和移除行为。
- 单元测试最近记录排序、数量限制和菜单标题压缩。
- 组件测试书签栏和管理页输出 favicon，并保留无图标回退。
- `pnpm test`、`pnpm typecheck`、`pnpm build` 通过。
- Electron 实机验证两个二级菜单、最近项、新标签和唯一管理页。
