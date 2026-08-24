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

03 的当前桌面实现技术基线为 GPUI + gpui-component，采用标题栏内顶部标签页、紧凑 Space 切换和 Space 书签栏，具体工程约束以同号详设为准。
04 定义面向 Rust 合作方的公开 SDK、独立 Runtime 和版本化 IPC 长期交付边界。
05 定义参考浏览器 V4 的实施范围，从稳定类型提取开始，依次交付版本化 IPC、Renderer 隔离、macOS 沙箱、基础会话和 Flexbox。
06 定义 V5 的 `archetype-sdk 0.1` 开发者预览，以公开 RGBA 帧和异步 Runtime 生命周期完成第一个 UI 框架无关接入闭环。
