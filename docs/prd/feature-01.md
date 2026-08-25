# Feature 01 跨平台窗口标题栏间距 PRD

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-01` |
| 状态 | 已实现 |
| 对应详设 | [feature-01.md](../detailed-design/feature-01.md) |
| 所属规范 | [01 Archetype Chromium 浏览器](01-Archetype-Chromium浏览器-PRD.md) |

## 1. 背景

标签栏与 macOS traffic lights 间距过小，且统一使用 macOS 的 `hiddenInset` 窗口样式无法正确适配 Windows 窗口控制按钮。

## 2. 需求与验收

| ID | 需求 | 验收标准 |
| --- | --- | --- |
| F01-01 | macOS 标签安全区 | traffic lights 保持原生可操作，最后一个按钮与首个标签之间至少保留 16px 视觉间距 |
| F01-02 | Windows 窗口控制 | 使用原生最小化、最大化和关闭按钮；标签与三键区域不重叠 |
| F01-03 | Windows 主题 | 原生窗口控制区颜色随 system/light/dark 设置更新 |
| F01-04 | Linux 回退 | 使用系统标题栏，不在标签栏中预留 macOS 或 Windows 控制区 |
| F01-05 | 窄窗口 | 在应用最小宽度下，标签可横向滚动，新建标签按钮和系统窗口按钮仍可操作 |
| F01-06 | Windows 分发 | 提供 NSIS 和 ZIP 构建命令，产物使用 Archetype 品牌图标 |

## 3. 非目标

- 本 Feature 不提供 Windows 代码签名、自动更新或跨平台 CI 产物发布。
- 不自绘 macOS traffic lights 或 Windows 三键。
