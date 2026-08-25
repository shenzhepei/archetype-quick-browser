# Feature 05 地址栏站点安全信息详细设计

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-05` |
| 状态 | 已实现 |
| 对应 PRD | [feature-05.md](../prd/feature-05.md) |
| 主要模块 | `src/main/site-security.ts`、`src/main/controller.ts`、`src/main/index.ts`、`src/renderer/browser/Toolbar.tsx` |

## 1. 状态模型

共享 `SiteInfo` 包含当前 URL、origin、连接状态、可选证书摘要和权限记录。连接状态为 `secure`、`insecure`、`local`、`internal` 或 `none`。权限记录状态为 `granted` 或 `blocked`。

`BrowserState.siteInfo` 只对应活动标签。Renderer 不提交 URL 或 origin，避免伪造其他站点信息。

## 2. 证书采集

`SiteSecurityService` 安装在 `persist:archetype` session：

- `setCertificateVerifyProc` 接收 Chromium 的 `certificate`、`validatedCertificate`、`verificationResult`、`errorCode` 和 `isIssuedByKnownRoot`。
- 按 hostname 保存最小证书摘要，不持久化 PEM 数据。
- 回调始终使用 `-3`，交还 Chromium 默认结果，不接受或拒绝原本不同的证书。
- HTTPS 只有在 Chromium `errorCode === 0` 或兼容成功结果 `OK`/`net::OK` 时标记 `secure`；尚未取得摘要时显示正在验证，不宣称证书有效。

证书详情使用 Electron 原生 message box，显示 subject、issuer、有效期、SHA 指纹及 trusted-root 状态，不创建可被网页覆盖的 React 浮层。

## 3. 权限状态

现有 `setPermissionRequestHandler` 委托给安全服务。服务按 requesting origin 记录实际请求的 permission 为 `blocked`，继续执行 `callback(false)`。不将 permission check 当成用户请求，避免仅探测 API 就产生误导记录。

权限状态目前为进程内只读记录。后续授权 Feature 应将 granted/blocked 决策持久化并同时实现 `setPermissionCheckHandler` 与 request handler。

## 4. 地址栏与菜单

地址栏布局增加固定 `32px` 站点信息按钮。安全 HTTPS 使用锁形图标，HTTP 使用警告图标，内部/本地页使用信息图标，无活动站点使用默认地球图标。

点击按钮通过白名单 `browser:show-site-info` IPC 请求当前活动标签菜单。主进程生成固定原生菜单：连接摘要、证书入口、权限标题和权限状态列表。坐标进行 finite 与窗口 bounds 校验。

## 5. 验证

- 纯函数测试 URL 状态分类和 origin 归一化。
- Toolbar 组件测试站点信息按钮 IPC。
- `pnpm test`、`pnpm typecheck`、`pnpm build` 通过。
- 真实 Electron 验证 HTTPS 证书摘要、HTTP 不安全状态和权限空状态。
