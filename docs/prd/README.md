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

03 的当前桌面实现技术基线为 GPUI + gpui-component，具体工程约束以同号详设为准。
