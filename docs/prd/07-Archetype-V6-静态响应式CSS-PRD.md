# Archetype V6 静态响应式 CSS 产品需求文档（PRD）

| 项 | 内容 |
|----|------|
| 规范号 | 07 |
| 版本 | V6 / `0.6.0` |
| 状态 | 已完成 |
| 日期 | 2026-08-24 |
| 对应详设 | [07-Archetype-V6-静态响应式CSS详设.md](../detailed-design/07-Archetype-V6-静态响应式CSS详设.md) |

---

## 1. 产品目标

V6 将 Archetype 从基础盒模型和 Flexbox 推进到可用于常见静态响应式页面的 CSS 子集。页面作者应能使用设计令牌、宽度断点、完整的基础 Flex item 排序与尺寸，以及脱离普通流的基础定位，而不需要针对 Archetype 复制一套固定宽度样式。

V6 不宣称完整实现 CSS。每项能力必须经过解析、计算样式、布局、绘制和固定截图验证后才进入公开支持矩阵；未知或超出范围的 CSS 继续聚合诊断并稳定降级。

## 2. 用户与场景

- 页面作者需要使用 CSS 自定义属性集中维护颜色、间距和尺寸。
- 桌面与 SDK 集成方需要同一静态文档在不同视口宽度下选择确定的响应式规则。
- 应用界面需要用 Flex item 顺序、basis 和基础定位实现工具栏、徽标、覆盖标签及固定尺寸面板。
- Archetype 维护者需要机器支持矩阵准确区分 V6 已支持能力与 Grid、动画等后续范围。

## 3. V6 必须交付

### 3.1 CSS 变量

- 接受以 `--` 开头的自定义属性，按层叠和继承生成每个节点的变量环境。
- 在已支持属性中解析 `var(--name)` 和单层 fallback `var(--name, fallback)`。
- 未定义、循环、超深或替换后无效的值不生效，并产生去重诊断；不得 panic 或无限递归。

### 3.2 宽度媒体查询

- 支持 `@media all`、`screen`、`(min-width: <length>)`、`(max-width: <length>)` 以及用 `and` 连接的宽度区间。
- 媒体规则在计算样式前按调用方视口宽度过滤；桌面 Browser 与 Runtime/SDK 必须得到相同结果。
- `print`、高度查询、用户偏好、容器查询和任意复杂布尔表达式保持不支持并给出诊断。

### 3.3 Flex item

- 支持 `flex-basis` 的 `auto`、像素、`em`、`rem` 和百分比值。
- 支持整数 `order`，同 order 保持文档顺序；row/column reverse 在排序后反转主轴。
- grow、shrink、basis、gap、wrap 和 alignment 组合保持确定且不生成负尺寸。

### 3.4 静态定位

- 支持 `position: static | relative | absolute` 与 `top/right/bottom/left` 长度或百分比偏移。
- relative 元素保留普通流位置后做视觉偏移；absolute 元素脱离普通流，并相对最近的非 static 祖先内容框定位，否则相对初始包含块。
- 支持整数 `z-index` 的稳定绘制顺序；相同层级保持文档顺序。

### 3.5 兼容与发布

- 工作区版本升级到 `0.6.0`，SDK 保持 `0.1.x`，Protocol 主版本保持 v4。
- 更新机器 HTML/CSS 支持矩阵、双语 README、V6 验收文档和发行归档。
- MSRV 保持 Rust `1.85`，V4/V5 Runtime、沙箱、SDK 和发布能力不得回退。

## 4. 验收标准

| 维度 | 验收标准 |
|------|----------|
| 变量 | 继承、覆盖、fallback、无效值和循环测试通过；替换结果进入计算样式 |
| 媒体 | 至少 320、768、1280 三种宽度产生预期规则选择，Browser 与 Runtime 一致 |
| Flex | basis、order、reverse、grow/shrink 和 wrap 组合布局测试通过 |
| 定位 | relative 保留流、absolute 脱流、祖先包含块、百分比 offset 和 z-index 测试通过 |
| 金样 | 新增至少 12 个 V6 页面及对应 macOS 截图，差异阈值不超过 0.5% |
| 降级 | 不支持的媒体条件、Grid、动画和无效变量产生聚合诊断且页面仍可读 |
| 回归 | 原 50 张截图、全工作区测试、严格 Clippy、rustdoc、覆盖率、沙箱和 SDK 验收通过 |

## 5. 明确不进入 V6

- CSS Grid、subgrid、multicolumn、float 和 shape-outside。
- transition、animation、关键帧、滤镜、3D transform 和 GPU 合成。
- sticky/fixed 定位、复杂 stacking context、mix-blend-mode 和 isolation。
- 完整媒体查询 Level 4、容器查询、打印布局和用户偏好查询。
- 完整伪类/伪元素、CSSOM、JavaScript 驱动 DOM mutation 和运行时动画。

## 6. 实施顺序

| 阶段 | 交付 | 退出条件 |
|------|------|----------|
| A 规格 | 07 PRD、详设、索引和矩阵目标 | 文档成对且范围无占位 |
| B 解析 | 自定义属性、var、结构化 media AST | parser/style 单元测试通过 |
| C Flex | basis、order 与排序后的主轴布局 | row/column/wrap 组合测试通过 |
| D 定位 | relative/absolute offset 与 z-index | layout/paint 测试通过 |
| E 金样 | 12 个页面、截图和矩阵证据 | 固定截图通过 |
| F 发布 | `0.6.0` 文档、归档和全量验收 | V6 全部验收项通过 |

## 7. 发布条件

V6 只有在上述 CSS 能力从 parser 到 Runtime 栅格均有测试证据、机器矩阵不再把已实现项标为 unsupported、原 50 张 V4 截图无回退，并且 Grid/动画等非目标仍被诚实标记后才能标记完成。

## 8. 相关文档

- [V6 静态响应式 CSS 详细设计](../detailed-design/07-Archetype-V6-静态响应式CSS详设.md)
- [V5 Rust SDK 预览 PRD](./06-Archetype-V5-Rust-SDK预览-PRD.md)
- [V4 安全运行时 PRD](./05-Archetype-V4-安全运行时-PRD.md)
