# Feature 03 GitHub Release 版本检查详细设计

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-03` |
| 状态 | 已实现 |
| 对应 PRD | [feature-03.md](../prd/feature-03.md) |
| 主要模块 | `src/main/release.ts`、`src/main/index.ts`、`src/preload/index.ts`、`src/renderer/browser/InternalPage.tsx` |

## 1. 数据模型

共享 `ReleaseStatus` 包含安装包当前版本、可选最新版本、可选 Release URL、检查时间及状态：`up-to-date`、`update-available`、`no-release`、`unavailable`。

## 2. 主进程检查

`ReleaseService` 使用 Electron `net.fetch` 请求 GitHub REST API 的 latest Release endpoint。请求设置 GitHub JSON Accept header、固定 User-Agent 和超时信号，不携带用户 Cookie 或 token。

- 当前版本来自 `app.getVersion()`。
- HTTP 404 映射为 `no-release`。
- 成功响应只接收合法 `tag_name`，Release URL 必须以仓库官方 Releases 路径开头。
- 版本比较去除 `v` 前缀，比较 major/minor/patch；正式 Release 高于当前版本时为 `update-available`。
- 网络、超时、非法 JSON 或非法版本映射为 `unavailable`，不向 Renderer 暴露内部错误细节。

检查结果短期缓存在主进程；用户点击重新检查时强制刷新。打开 Release 不接受 Renderer URL 参数，只使用服务缓存中已验证的 URL。

## 3. IPC 与 UI

preload 增加 `checkForUpdates(force?)` 和 `openLatestRelease()` 白名单命令。关于页首次显示时自动检查，显示当前版本、检查状态和最新版本；加载时显示 spinner，发现新版时显示外部打开命令，其余状态提供重新检查。

静态 Web preview 返回 `unavailable` 演示状态，不发送真实 GitHub 请求。

## 4. 验证

- 纯函数测试版本比较和 tag 归一化。
- About 组件测试加载与状态渲染。
- `pnpm test`、`pnpm typecheck`、`pnpm build` 通过。
- 真实 Electron 关于页验证当前版本与 GitHub 无 Release 状态；发布 Release 后验证新版判断和外部链接。
