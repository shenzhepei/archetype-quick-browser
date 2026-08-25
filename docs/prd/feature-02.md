# Feature 02 标签性能与站点兼容性修复 PRD

| 字段 | 内容 |
|------|------|
| Feature ID | `feature-02` |
| 状态 | 已完成 |
| 来源 | 双标签切换慢、百度正文缺失、favicon 消失、新标签不是空白页 |
| 对应详设 | [feature-02.md](../detailed-design/feature-02.md) |

## 1. 背景

Feature 01 完成浏览器外壳后，真实使用暴露出四个回归：普通标签切换错误地执行页面休眠与重载；部分站点按压缩能力和 User-Agent 返回不同文档；SVG favicon 或休眠后的 favicon 会消失；新标签仍导航到开发 fixture。本 Feature 将这些行为修正为稳定的桌面浏览器语义。

## 2. 功能需求

| ID | 需求 | 验收标准 |
|----|------|----------|
| F02-01 | 两个普通标签快速切换 | 两标签切换不生成休眠快照、不删除显示列表、不重新加载网络页面 |
| F02-02 | 保留有界内存控制 | 达到 8 个常驻渲染页后才允许在切换时休眠旧页；脏表单保护继续生效 |
| F02-03 | 百度主文档可加载 | HTTP 支持 gzip、Brotli、deflate、zstd，并发送带 Archetype 标识的桌面浏览器 User-Agent |
| F02-04 | favicon 不因格式或恢复流程消失 | 支持 PNG/JPEG/ICO 归一化和有界 SVG；显式图标失败后尝试 `/favicon.ico`；休眠恢复走完整异步加载管线 |
| F02-05 | 新标签是真正的空白页 | 新标签 URL 为 `about:blank`，不读取 fixture、不发起网络请求、不显示“打开页面”提示 |

## 3. 兼容与安全要求

- 解压后的响应仍受原有字节上限约束，不能用压缩绕过资源预算。
- User-Agent 必须包含 `Archetype/<crate-version>`，版本随发布自动更新。
- SVG favicon 受 1 MiB 下载上限和同源最终 URL 校验约束。
- 百度 CDN 上的跨域样式或图片仍可被 V4 同源策略拒绝，并在诊断中说明。
- 不通过执行百度的 JavaScript 完成兼容；主文档必须能够静态进入 HTML/CSS 管线。

## 4. 非目标

- 不在本 Feature 中开放跨域被动资源、JavaScript 或完整 CSS。
- 不承诺所有站点对 User-Agent 的服务端分流结果一致。
- 不实现无限标签常驻或跨启动持久化完整 DOM/显示列表。
- 不将 `about:blank` 扩展为通用 `about:` 页面系统。

## 5. 验收证据

- 两标签与 8 标签阈值的纯逻辑回归测试。
- `about:blank` URL 和不加载判定测试。
- 本地 HTTP gzip 响应与 User-Agent 断言。
- SVG favicon 保留、PNG 默认 favicon 和跨域拒绝测试。
- 百度在线探针确认显示列表存在文本和至少一张已加载同源图片。
- 严格 Clippy、全工作区测试、72 页截图、支持矩阵和 SDK 兼容矩阵通过。
