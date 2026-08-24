# Archetype V7 Grid 与视觉 CSS 详细设计

| 项 | 内容 |
|----|------|
| 规范号 | 08 |
| 对应 PRD | [08-Archetype-V7-Grid与视觉CSS-PRD.md](../prd/08-Archetype-V7-Grid与视觉CSS-PRD.md) |
| 版本 | V7 / `0.7.0` |
| 状态 | 已完成 |
| 日期 | 2026-08-25 |

---

## 1. 冻结决策

| ADR | V7 决策 | 后续边界 |
|-----|---------|----------|
| Grid | 有界显式列 + 行优先自动放置 | 完整 Grid track sizing 后移 |
| Track parser | 结构化 token 解析，不拆 CSS 原始字符串 | `minmax()` 与命名线后移 |
| 视觉效果 | DisplayList 携带确定参数，Browser 与 raster 共用语义 | GPU 合成与动画后移 |
| Alpha | 计算样式 opacity 乘入每个绘制命令 | 独立合成层后移 |
| 阴影 | 单层外阴影，CPU 栅格有界模糊 | 多层/inset/spread 后移 |
| 兼容 | workspace/runtime `0.7.0`，SDK `0.1.x`，Protocol v4 | 不新增跨进程能力字段 |

## 2. 数据流

```text
CSS tokens
  -> arch-css Declaration
  -> arch-style ComputedStyle
       |- display/grid tracks/gaps
       `- radius/opacity/shadow/text decoration
  -> arch-layout
       |- resolve fixed and percentage tracks
       |- distribute remaining width to fr tracks
       `- row-major child placement
  -> arch-paint DisplayCommand
  -> Browser GPUI / archetype-raster RGBA
```

## 3. 样式模型

`ComputedStyle` 新增 `grid_template_columns`、`row_gap_px`、`column_gap_px`、`border_radius_px`、`opacity`、`box_shadow` 和 `text_decoration`。Grid 轨道使用公开、可序列化的枚举表示固定长度、百分比和弹性份额。

解析上限：

- 每个 Grid 最多 64 列。
- `repeat()` 次数为 `1..=64`，展开后仍不得超过 64 列。
- `fr` 必须有限且大于零；长度和百分比必须非负。
- opacity 无效时忽略声明，不做隐式 clamp。
- 阴影 blur 上限 64 px，避免 CPU 栅格出现无界工作量。

## 4. Grid 布局

容器内容宽记为 `W`，列间距总和记为 `G`。先从 `W-G` 扣除固定轨道和百分比轨道，剩余值 `R=max(0, remaining)` 按每个 `fr` 权重分配。没有列定义或定义全部无效时退化为单列。

子节点按文档顺序分组，每组数量不超过列数。每个项目用对应轨道宽作为 containing block 完成现有 block 布局；一行全部完成后取最大项目高度作为行高，将该行较矮项目保留自身高度，再推进 `row-gap`。absolute 子节点继续走 V6 pending absolute 队列，不占 Grid 单元。

V7 不实现跨列/跨行，因此每个普通流子节点恰好占一个单元。Grid 容器的自然高度为所有行高与行间距之和。

## 5. 绘制模型

`LayoutBox` 和 `DisplayCommand` 传递圆角、opacity 与阴影。Box 命令先绘制阴影，再绘制圆角背景和边框；文本命令在基线附近绘制 underline 或 line-through；图片 alpha 与同盒 opacity 一致。

- Browser GPUI 使用可表达的圆角、透明度与阴影参数。
- 确定性 rasterizer 使用整数像素有界算法；模糊半径转换为最大 64 px 的 separable box blur。
- clip 与圆角相交，不能扩大 V6 overflow clip。
- opacity 只作用当前盒对应命令；V7 不创建 CSS compositing group，因此嵌套 opacity 不声明完整规范等价。

## 6. 诊断与降级

- 已识别但值无效的属性沿用声明无效语义，不覆盖已有 winner。
- Grid 轨道超限、未知函数和复杂阴影各聚合为稳定诊断。
- `background` 只有单一颜色成功解析时进入计算样式，否则继续报告 unsupported property。
- 动画、filter、transform 与复杂 Grid 保持机器矩阵 unsupported。

## 7. 测试与语料

- `arch-css`：新属性支持集合与复杂值诊断。
- `arch-style`：轨道解析、repeat 上限、gap 层叠、视觉值合法性。
- `arch-layout`：固定/百分比/fr、混合轨道、多行、不同项目高度。
- `arch-paint`：新参数稳定进入命令，alpha 和装饰不丢失。
- `archetype-raster`：圆角角像素、阴影范围、文本装饰、图片 alpha。
- Browser/Runtime：新增 10 页语料；原 62 页截图不变。

## 8. 实施切片

| 切片 | 代码范围 | 验证 |
|------|----------|------|
| A1 | 08 规格与索引 | 文档配对、链接 |
| B1 | CSS 支持集合与 ComputedStyle | parser/style tests |
| C1 | Grid 轨道与自动放置 | layout tests |
| D1 | DisplayList 视觉参数 | paint tests |
| D2 | Browser/raster 绘制 | RGBA tests |
| E1 | 10 页语料与支持矩阵 | snapshot/runtime tests |
| F1 | 版本、README、验收文档 | workspace acceptance |

## 9. 完成定义

V7 完成必须满足 08 PRD 验收标准。每个标为 supported 的能力至少绑定一个计算或布局测试和一个 Browser/Runtime 输出证据；任何仅部分实现的 Grid 或视觉语义必须标记 partial 或保持 unsupported。

## 10. 相关文档

- [V7 Grid 与视觉 CSS PRD](../prd/08-Archetype-V7-Grid与视觉CSS-PRD.md)
- [V6 静态响应式 CSS 详设](./07-Archetype-V6-静态响应式CSS详设.md)
- [V5 Rust SDK 预览详设](./06-Archetype-V5-Rust-SDK预览详设.md)
