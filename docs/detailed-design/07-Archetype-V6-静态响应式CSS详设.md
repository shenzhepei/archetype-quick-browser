# Archetype V6 静态响应式 CSS 详细设计

| 项 | 内容 |
|----|------|
| 规范号 | 07 |
| 对应 PRD | [07-Archetype-V6-静态响应式CSS-PRD.md](../prd/07-Archetype-V6-静态响应式CSS-PRD.md) |
| 版本 | V6 / `0.6.0` |
| 状态 | 已完成 |
| 日期 | 2026-08-24 |

---

## 1. 冻结决策

| ADR | V6 决策 | 后续边界 |
|-----|---------|----------|
| CSS 范围 | 静态响应式子集，不追求规范全集 | Grid 与动态效果进入 V7 |
| Parser | 继续使用 cssparser token/规则 API，新增结构化 MediaCondition | 禁止字符串截取嵌套规则 |
| 变量 | 每节点继承后的有界 map；使用时递归替换 | CSSOM 与注册属性后移 |
| 视口 | style 阶段显式接收 viewport width | 不读取全局窗口状态 |
| 定位 | layout 生成普通流与 positioned 两类盒 | fixed/sticky 后移 |
| 层级 | 单一布局树内按整数 z-index 稳定排序 | 完整 stacking context 后移 |
| 兼容 | Runtime 与 workspace `0.6.0`，SDK `0.1.x`，Protocol v4.1 | 后续协议能力不在 V6 必需范围 |

## 2. 数据流

```text
CSS bytes
  -> arch-css Stylesheet { rules, media condition, diagnostics }
  -> arch-style style_document_for_viewport(document, sheet, width)
       |- cascade custom properties
       |- inherit variable environment
       `- resolve var() before typed property parsing
  -> arch-layout
       |- ordered FlexSeed { basis, order, document index }
       |- normal-flow boxes
       `- positioned boxes { containing block, offsets, z-index }
  -> arch-paint stable z-order DisplayList
  -> Browser / Runtime / SDK RGBA
```

## 3. CSS Parser

`arch-css::Rule` 新增 `media: Option<MediaCondition>`。顶层普通规则为 `None`；`@media` 块内规则携带合并后的条件。V6 不允许嵌套 media 超过 8 层。

```rust
pub struct MediaCondition {
    pub medium: MediaType,
    pub min_width_px: Option<f32>,
    pub max_width_px: Option<f32>,
}
```

- 长度接受 `px`、`em`、`rem`；media 中 `em/rem` 固定按初始 16 px 计算。
- 同一条件重复下限取较大值、重复上限取较小值；下限大于上限时永不匹配。
- 不支持的 at-rule 整块忽略并记录一次去重诊断。
- 自定义属性名区分大小写，普通属性名继续 ASCII 小写。

## 4. 层叠与变量

每个元素先完成声明 winner 选择，再从父节点复制变量 map，最后应用当前节点获胜的 `--*` 声明。普通属性值在 typed parse 前执行 `var()` 替换。

- 最大变量数：每节点 256。
- 最大展开深度：16。
- 最大展开后值：64 KiB。
- 递归栈中再次遇到同名变量即判定循环。
- fallback 只在变量缺失或无效时使用；fallback 自身允许继续包含 `var()`。
- 变量失败仅使对应普通声明无效，不抹除继承值或 UA 默认值。

保留 `style_document` 作为兼容包装，默认使用 1280 px；Browser 与 Runtime 改用 `style_document_for_viewport` 并传入实际视口。

## 5. Flex item

`ComputedStyle` 新增：

```rust
pub flex_basis: Option<ComputedLength>,
pub order: i32,
```

- basis 为 `auto` 时沿用现有 width/height 或内容测量。
- row 主轴百分比相对容器内容宽，column 主轴百分比仅在容器有确定高度时解析，否则回退 auto。
- Flex 子节点按 `(order, document_index)` 稳定排序，再应用 direction reverse。
- grow/shrink 继续使用加权 basis，最终尺寸夹到非负有限值。

## 6. 定位布局

`ComputedStyle` 新增 `position`、四个可选 offset 和 `z_index`。`LayoutBox` 新增 `z_index` 与单调 `paint_order`。

- static：沿用现有布局。
- relative：先完整参与普通流，再在该节点及其后代生成的盒范围上应用视觉偏移；cursor 不随偏移变化。
- absolute：不推进父 cursor。先以最近 positioned 祖先的 content rect 为 containing block；没有时使用 `(0, 0, viewport_width, viewport_height)`。
- 同轴同时指定两端且尺寸 auto 时由两端推导尺寸；已有明确尺寸时 start 端优先。
- 百分比水平 offset 相对包含块宽，垂直 offset 相对包含块高；初始包含块高使用调用方 viewport height，当前 Runtime 接口补充该值。
- paint 按 `(z_index, paint_order)` 稳定排序。V6 不创建嵌套 stacking context。

## 7. 调用边界

- Browser `render_document` 将实际 viewport width 传入 style/layout。
- Protocol 已携带宽高；Runtime 使用两者做 media 与 initial containing block。
- SDK API 不变化，`PageOptions` 的宽高直接驱动 V6 响应式结果。
- 支持矩阵版本更新为 `0.6.0`，每个新 feature id 至少绑定一个单元测试和一个金样测试。

## 8. 诊断

新增稳定诊断类别：

- `ignored unsupported CSS at-rule: <name>`
- `ignored unsupported media condition: <condition>`
- `ignored invalid CSS variable: <name>`
- `ignored unresolved CSS variable in property: <property>`

诊断按类别与标识去重，不包含完整 HTML、CSS 内容、文件路径或凭据。

## 9. 测试

- `arch-css`：media token 解析、嵌套块、范围合并、未知 at-rule、custom property 保留。
- `arch-style`：变量继承/覆盖/fallback/循环/上限、视口规则选择、typed value 替换。
- `arch-layout`：basis/order/reverse/wrap、relative 流位置、absolute 包含块与百分比。
- `arch-paint`：负/零/正 z-index 与文档顺序。
- Browser/Runtime：320/768/1280 响应式结果一致；原 50 页加 12 页 V6 corpus。
- 回归：workspace fmt、Clippy、test、rustdoc、LCOV、sandbox、entitlement、V5 SDK partner example。

## 10. 实施切片

| 切片 | 代码范围 | 验证 |
|------|----------|------|
| A1 规格 | 07 PRD、详设和索引 | 编号、配对、链接 |
| B1 parser | custom properties 与 media AST | arch-css tests |
| B2 style | viewport cascade 与 var resolver | arch-style tests |
| C1 flex | basis、order 与主轴测量 | arch-layout tests |
| D1 position | relative/absolute offsets | arch-layout tests |
| D2 paint | z-index 稳定排序 | arch-paint tests |
| E1 corpus | 12 页面、截图、矩阵 | Browser/Runtime tests |
| F1 release | 版本、README、验收与归档 | 全工作区验收 |

## 11. 完成定义

V6 完成必须满足 07 PRD 全部验收项，并保持 V4 Runtime/沙箱和 V5 SDK 行为。任何只进入 parser、未进入实际 Runtime 栅格的属性不得在支持矩阵中标为 supported。

完成证据包括 62 页固定截图、全工作区测试与严格 Clippy、真实 Runtime/SDK 双视口测试、100 次 Runtime 重启、沙箱与 entitlement 探针，以及 60 秒内 3,720 次页面加载的 CPU/RSS 趋势报告。

## 12. 相关文档

- [V6 静态响应式 CSS PRD](../prd/07-Archetype-V6-静态响应式CSS-PRD.md)
- [V5 Rust SDK 预览详设](./06-Archetype-V5-Rust-SDK预览详设.md)
- [V4 安全运行时详设](./05-Archetype-V4-安全运行时详设.md)
