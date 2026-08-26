# Feature 11 历史过滤与嵌套书签文件夹详细设计

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-11` |
| 状态 | 已实现 |
| 对应 PRD | [feature-11.md](../prd/feature-11.md) |
| 主要模块 | `src/shared/browser.ts`、`src/shared/bookmark-tree.ts`、`src/main/controller.ts`、`src/main/store.ts`、`src/renderer/browser/InternalPage.tsx` |

## 1. 历史记录时序

共享 `isRecordableHistoryEntry(title, url)` 只接受 HTTP/HTTPS URL 和非空、非 `New tab`/`新标签页` 标题。Controller 初始化时过滤旧记录。

`did-navigate` 仍更新导航状态并尝试记录已有有效标题；`page-title-updated` 更新标签标题后再次记录。若最近一条记录 URL 相同且间隔不超过 5 秒，则更新标题和时间，不新增重复项。这样既等待真实标题，也兼容不触发标题事件但已恢复有效标题的页面。

## 2. 文件夹数据

`Bookmark` 增加可选 `parentId`，新增 `BookmarkFolder`：`id`、`name`、可选 `parentId`、`createdAt`。`PersistedState.bookmarkFolders` 缺失时迁移为空数组，旧书签自然位于根目录。

Controller 创建文件夹时裁剪名称并限制为 80 字符，父 ID 必须存在。移动书签只接受存在的书签和目标文件夹。删除文件夹先通过树遍历收集全部后代，再一次性删除文件夹集合和其中书签。

## 3. 树构建

纯函数 `buildBookmarkTree` 从扁平书签和文件夹构建根节点列表。遍历维护 visited 集合，循环或悬空父引用回退到根目录，避免递归失控或数据不可见。

原生书签菜单包含最近书签子菜单，之后递归输出文件夹和根书签。每个文件夹子菜单递归包含子文件夹与直接书签，空文件夹显示禁用空状态。

## 4. 管理页面

书签管理页使用侧栏树选择当前文件夹，主区域只展示其直接子文件夹和书签。页头“新建文件夹”切换内联名称输入；提交时使用当前文件夹作为父级。

每条书签提供目标文件夹选择控件。文件夹删除按钮先通过本地确认框明确递归删除影响，再调用固定 IPC。状态发布后若当前目录已被删除，页面回到根目录。

## 5. IPC

新增固定 IPC：创建文件夹、删除文件夹、移动书签。Renderer 只能发送名称和实体 ID；Controller 验证所有引用，不接受任意持久化对象或菜单模板。

## 6. 验证

- 单元测试历史占位过滤、同 URL 更新和最近历史防御过滤。
- 单元测试多级树、悬空父级和循环数据处理。
- 组件测试创建子文件夹、移动书签和删除确认。
- `pnpm test`、`pnpm typecheck`、`pnpm build` 通过。
- Electron 实机验证多级目录管理和递归原生菜单。
