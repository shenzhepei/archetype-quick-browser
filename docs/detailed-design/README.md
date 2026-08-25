# Detailed Design 目录

放**怎么做**的工程设计：架构、模块、状态机、接口与阶段拆分。

| 规范号 | 详设 | 对应 PRD | 说明 |
|--------|------|----------|------|
| 01 | [Archetype 总体详设](./01-Archetype-总体详设.md) | [01 Archetype PRD](../prd/01-Archetype-PRD.md) | 引擎、资源模型、存储、UI、阶段 |
| 02 | [Archetype 扩展系统详设](./02-Archetype-扩展系统详设.md) | [02 扩展系统 PRD](../prd/02-Archetype-扩展系统-PRD.md) | 零信任扩展 ZTE |
| 03 | [Archetype V3 详设](./03-Archetype-V3-详设.md) | [03 Archetype V3 PRD](../prd/03-Archetype-V3-PRD.md) | 标题栏标签页、紧凑 Space、书签栏与第一个可实施版本的验收基线 |
| 04 | [Archetype Rust SDK 与 Runtime 详设](./04-Archetype-Rust-SDK与Runtime详设.md) | [04 Rust SDK 与 Runtime PRD](../prd/04-Archetype-Rust-SDK与Runtime-PRD.md) | Rust-only SDK、版本化 IPC、独立 Runtime、安全与交付边界 |
| 05 | [Archetype V4 安全运行时详设](./05-Archetype-V4-安全运行时详设.md) | [05 Archetype V4 安全运行时 PRD](../prd/05-Archetype-V4-安全运行时-PRD.md) | 稳定类型、内部 IPC、Renderer 隔离、macOS 沙箱、基础会话与 Flexbox |
| 06 | [Archetype V5 Rust SDK 预览详设](./06-Archetype-V5-Rust-SDK预览详设.md) | [06 Archetype V5 Rust SDK 预览 PRD](../prd/06-Archetype-V5-Rust-SDK预览-PRD.md) | SDK 0.1、异步 Runtime 生命周期、公开 RGBA 帧与合作方示例 |
| 07 | [Archetype V6 静态响应式 CSS 详设](./07-Archetype-V6-静态响应式CSS详设.md) | [07 Archetype V6 静态响应式 CSS PRD](../prd/07-Archetype-V6-静态响应式CSS-PRD.md) | 变量、宽度媒体查询、Flex item 与静态定位 |
| 08 | [Archetype V7 Grid 与视觉 CSS 详设](./08-Archetype-V7-Grid与视觉CSS详设.md) | [08 Archetype V7 Grid 与视觉 CSS PRD](../prd/08-Archetype-V7-Grid与视觉CSS-PRD.md) | 有界基础 Grid、圆角、透明度、单层阴影与文本装饰 |

## 补充 Feature

| Feature ID | 详设 | 对应 PRD | 状态 | 说明 |
|------------|------|----------|------|------|
| feature-01 | [浏览器外壳与本地安全体验详设](./feature-01.md) | [feature-01 PRD](../prd/feature-01.md) | 已完成 | 地址栏与标签结构、外观状态、favicon 管线和 Debug/Release 密钥边界 |
| feature-02 | [标签性能与站点兼容性修复详设](./feature-02.md) | [feature-02 PRD](../prd/feature-02.md) | 已完成 | 常驻页阈值、异步恢复、HTTP 内容协商、SVG favicon 和空白页 |
| feature-03 | [标签页加载状态提示详设](./feature-03.md) | [feature-03 PRD](../prd/feature-03.md) | 已完成 | 每标签 loading 环、favicon/默认图标叠放与加载生命周期 |
| feature-04 | [浏览器内部页与历史记录详设](./feature-04.md) | [feature-04 PRD](../prd/feature-04.md) | 已完成 | schema v5、三点菜单、历史与设置内部路由 |
| feature-05 | [Chromium 内核迁移详设](./feature-05.md) | [feature-05 PRD](../prd/feature-05.md) | 实施中 | CEF runtime、GPUI child view、标签事件、profile 与 Helper 打包 |

产品「做什么/不做什么」见 [`../prd/`](../prd/)。
