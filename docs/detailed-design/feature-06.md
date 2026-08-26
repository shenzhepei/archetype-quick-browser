# Feature 06 设置页指针反馈详细设计

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-06` |
| 状态 | 已实现 |
| 对应 PRD | [feature-06.md](../prd/feature-06.md) |
| 主要模块 | `src/renderer/styles/main.scss` |

## 1. 样式设计

在 `.settings-layout aside button` 和 `.segmented button` 规则中增加 `cursor: pointer`。两个选择器分别覆盖设置页侧栏中的外观、语言和关于菜单项，以及语言与主题的分段切换选项，保持其他按钮原有指针规则。

## 2. 验证

- 检查 SCSS 编译通过。
- 在设置页悬停左侧菜单项和语言切换选项时，系统指针显示为手型。
