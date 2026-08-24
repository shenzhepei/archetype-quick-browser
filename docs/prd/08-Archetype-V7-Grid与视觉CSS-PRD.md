# Archetype V7 Grid 与视觉 CSS 产品需求文档（PRD）

| 项 | 内容 |
|----|------|
| 规范号 | 08 |
| 版本 | V7 / `0.7.0` |
| 状态 | 已完成 |
| 日期 | 2026-08-25 |
| 对应详设 | [08-Archetype-V7-Grid与视觉CSS详设.md](../detailed-design/08-Archetype-V7-Grid与视觉CSS详设.md) |

---

## 1. 产品目标

V7 在 V6 静态响应式布局基础上补齐常见产品页、搜索页和工具界面的基础二维布局与高频视觉表达。页面作者可以使用有界 CSS Grid、圆角、透明度、单层阴影和文本装饰，而不需要将这些结构改写为 Flexbox 或图片。

V7 仍是静态、安全、确定性的 HTML/CSS 渲染器，不宣称完整 CSS 或 Web 平台兼容。所有公开支持项必须贯穿解析、计算样式、布局、绘制、Runtime 和固定语料；只被 parser 接受的属性不得标记为支持。

## 2. 用户与场景

- 搜索页和导航页需要用等宽或混合尺寸的列组织入口。
- 产品卡片和设置面板需要圆角、透明度和有界阴影表达层级。
- 文本链接和状态内容需要基础下划线或删除线。
- 维护者需要减少真实网站中 `border-radius`、`box-shadow`、`opacity`、`text-decoration` 和 `grid-*` 的重复诊断。

## 3. V7 必须交付

### 3.1 基础 Grid

- 支持 `display: grid`。
- 支持 `grid-template-columns` 的非负 `px`、百分比、`fr` 以及非嵌套 `repeat(<正整数>, <单轨道>)`。
- 支持 `gap`、`row-gap` 和 `column-gap` 的非负绝对长度。
- 子项按文档顺序自动放置到行优先网格；行高取该行最高项目。
- 百分比和固定轨道先分配，剩余宽度按 `fr` 比例分配，结果不得为负数或非有限值。

### 3.2 高频视觉 CSS

- 支持统一四角的 `border-radius` 非负长度，圆角同时裁剪背景和边框。
- 支持 `opacity: 0..1`，并乘入盒、文本和图片的最终 alpha。
- 支持单层、无 `inset`、无 spread 的 `box-shadow: <x> <y> <blur> <color>`。
- 支持 `text-decoration` 的 `none`、`underline` 和 `line-through`。
- `background` 简写在值为单一可解析颜色时等价于 `background-color`；复杂背景继续诊断并忽略。

### 3.3 兼容与发布

- workspace 与 Runtime 升级到 `0.7.0`；SDK 保持 `0.1.x`，Protocol v4 保持兼容。
- 更新机器支持矩阵、双语 README、V7 验收说明和确定性语料。
- V4 安全边界、V5 SDK 与 V6 响应式行为不得回退。

## 4. 验收标准

| 维度 | 验收标准 |
|------|----------|
| Grid | 固定、百分比、`fr`、`repeat()`、混合轨道和多行自动放置测试通过 |
| 间距 | `gap`、`row-gap`、`column-gap` 层叠和布局测试通过 |
| 视觉 | 圆角、透明度、单层阴影和文本装饰进入 DisplayList 与 RGBA 栅格 |
| 诊断 | 超界 repeat、嵌套 repeat、复杂背景、复杂阴影和动画稳定降级 |
| 语料 | 新增至少 10 个 V7 页面并通过固定截图阈值 |
| 回归 | 原 62 页、全 workspace 测试、严格 Clippy、rustdoc、Runtime 与 SDK 测试通过 |

## 5. 明确不进入 V7

- 显式 `grid-row`/`grid-column` 放置、span、命名线、隐式轨道控制、subgrid 和 masonry。
- `minmax()`、`auto-fit`、`auto-fill`、内容尺寸轨道和复杂 track sizing algorithm。
- 多层或 inset 阴影、spread、渐变、背景图片和多背景。
- 每角独立半径、椭圆半径、复杂边框样式和完整 stacking context。
- transition、animation、关键帧、filter、transform 和 GPU 合成。
- JavaScript、CSSOM、动态 DOM mutation 和运行时重排动画。

## 6. 实施顺序

| 阶段 | 交付 | 退出条件 |
|------|------|----------|
| A 规格 | 08 PRD、详设与索引 | 编号、配对和链接验证通过 |
| B 样式 | Grid/视觉属性解析与计算 | `arch-css`、`arch-style` 测试通过 |
| C 布局 | Grid 轨道分配与自动放置 | `arch-layout` 测试通过 |
| D 绘制 | 圆角、alpha、阴影、装饰 | paint/raster 测试通过 |
| E 语料 | 10 个 V7 页面与支持矩阵 | Browser/Runtime 金样通过 |
| F 发布 | `0.7.0` 文档与全量验收 | V7 全部验收项通过 |

## 7. 发布条件

V7 只有在 Grid 和视觉属性从 CSS 输入到 Runtime RGBA 输出均有自动化证据，支持矩阵与实际实现一致，原 62 页无回退，且动画与复杂 Grid 仍明确标记为不支持时才能标记完成。

## 8. 相关文档

- [V7 Grid 与视觉 CSS 详细设计](../detailed-design/08-Archetype-V7-Grid与视觉CSS详设.md)
- [V6 静态响应式 CSS PRD](./07-Archetype-V6-静态响应式CSS-PRD.md)
- [V5 Rust SDK 预览 PRD](./06-Archetype-V5-Rust-SDK预览-PRD.md)
