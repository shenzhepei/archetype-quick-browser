# 为何用 `prd` 而不是 `需求`

| 目录 | 适合放什么 |
|------|------------|
| **`docs/prd/`（已选）** | 产品需求文档（PRD）：愿景、用户价值、范围、非目标、里程碑验收——给产品/决策用 |
| `docs/需求/` | 更偏「需求条目池」、工单式拆分；颗粒更碎 |

本项目结论型文档统一放在 **prd**；工程怎么落地放在 [`../detailed-design/`](../detailed-design/)。

| 规范号 | PRD | 对应详设 |
|--------|-----|----------|
| 01 | [Archetype PRD](./01-Archetype-PRD.md) | [Archetype 总体详设](../detailed-design/01-Archetype-总体详设.md) |
| 02 | [扩展系统 PRD](./02-Archetype-扩展系统-PRD.md) | [扩展系统详设](../detailed-design/02-Archetype-扩展系统详设.md) |
| 03 | [Archetype V3 PRD](./03-Archetype-V3-PRD.md) | [Archetype V3 详设](../detailed-design/03-Archetype-V3-详设.md) |
| 04 | [Rust SDK 与 Runtime PRD](./04-Archetype-Rust-SDK与Runtime-PRD.md) | [Rust SDK 与 Runtime 详设](../detailed-design/04-Archetype-Rust-SDK与Runtime详设.md) |
| 05 | [Archetype V4 安全运行时 PRD](./05-Archetype-V4-安全运行时-PRD.md) | [Archetype V4 安全运行时详设](../detailed-design/05-Archetype-V4-安全运行时详设.md) |
| 06 | [Archetype V5 Rust SDK 预览 PRD](./06-Archetype-V5-Rust-SDK预览-PRD.md) | [Archetype V5 Rust SDK 预览详设](../detailed-design/06-Archetype-V5-Rust-SDK预览详设.md) |
| 07 | [Archetype V6 静态响应式 CSS PRD](./07-Archetype-V6-静态响应式CSS-PRD.md) | [Archetype V6 静态响应式 CSS 详设](../detailed-design/07-Archetype-V6-静态响应式CSS详设.md) |
| 08 | [Archetype V7 Grid 与视觉 CSS PRD](./08-Archetype-V7-Grid与视觉CSS-PRD.md) | [Archetype V7 Grid 与视觉 CSS 详设](../detailed-design/08-Archetype-V7-Grid与视觉CSS详设.md) |

## 补充 Feature

| Feature ID | PRD | 对应详设 | 状态 | 范围 |
|------------|-----|----------|------|------|
| feature-01 | [浏览器外壳与本地安全体验](./feature-01.md) | [feature-01 详设](../detailed-design/feature-01.md) | 已完成 | 地址栏、标签按钮、外观设置、favicon、百度访问与 Debug 密钥 |
| feature-02 | [标签性能与站点兼容性修复](./feature-02.md) | [feature-02 详设](../detailed-design/feature-02.md) | 已完成 | 标签常驻、HTTP 解压/UA、SVG favicon 与 `about:blank` |
| feature-03 | [标签页加载状态提示](./feature-03.md) | [feature-03 详设](../detailed-design/feature-03.md) | 已完成 | 标签内旋转 loading 环、favicon 缩放与默认网站图标 |
| feature-04 | [浏览器内部页与历史记录](./feature-04.md) | [feature-04 详设](../detailed-design/feature-04.md) | 已完成 | 三点主菜单、历史、设置与 `archetype://` 路由 |
| feature-05 | [Chromium 内核迁移](./feature-05.md) | [feature-05 详设](../detailed-design/feature-05.md) | 实施中 | CEF、现代 Web 平台、GPUI 嵌入与多进程打包 |

03 的当前桌面实现技术基线为 GPUI + gpui-component，采用标题栏内顶部标签页、紧凑 Space 切换和 Space 书签栏，具体工程约束以同号详设为准。
04 定义面向 Rust 合作方的公开 SDK、独立 Runtime 和版本化 IPC 长期交付边界。
05 定义参考浏览器 V4 的实施范围，从稳定类型提取开始，依次交付版本化 IPC、Renderer 隔离、macOS 沙箱、基础会话和 Flexbox。
06 定义 V5 的 `archetype-sdk 0.1` 开发者预览，以公开 RGBA 帧和异步 Runtime 生命周期完成第一个 UI 框架无关接入闭环。
07 定义 V6 的静态响应式 CSS 子集，覆盖变量、宽度媒体查询、完整基础 Flex item 与 relative/absolute 定位。
08 定义 V7 的有界基础 Grid 与高频视觉 CSS，覆盖圆角、透明度、单层阴影和文本装饰。
