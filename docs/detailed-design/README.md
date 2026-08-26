# 详细设计索引

| 规范号 | 详细设计 | 对应 PRD | 状态 | 范围 |
| --- | --- | --- | --- | --- |
| `01` | [Archetype Chromium 浏览器](01-Archetype-Chromium浏览器详设.md) | [PRD](../prd/01-Archetype-Chromium浏览器-PRD.md) | 实施中 | Electron 主进程、WebContentsView、Browser Chrome 和持久化 |

## Feature 补充需求

| Feature | 详细设计 | 对应 PRD | 状态 | 范围 |
| --- | --- | --- | --- | --- |
| `feature-01` | [跨平台窗口标题栏间距](feature-01.md) | [PRD](../prd/feature-01.md) | 已实现 | 平台窗口参数、安全区、主题联动和 Windows 构建 |
| `feature-02` | [网页上层主菜单](feature-02.md) | [PRD](../prd/feature-02.md) | 已实现 | 原生菜单、固定命令 IPC 与坐标校验 |
| `feature-03` | [GitHub Release 版本检查](feature-03.md) | [PRD](../prd/feature-03.md) | 已实现 | 主进程 Release 查询、安全 IPC 与关于页状态 |
| `feature-04` | [稳定标签宽度与标签菜单](feature-04.md) | [PRD](../prd/feature-04.md) | 已实现 | 标签宽度公式、上下文菜单 IPC 与批量关闭 |
| `feature-05` | [地址栏站点安全信息](feature-05.md) | [PRD](../prd/feature-05.md) | 已实现 | 证书验证采集、权限状态和站点信息菜单 |
| `feature-06` | [设置页指针反馈](feature-06.md) | [PRD](../prd/feature-06.md) | 已实现 | 设置菜单和分段切换控件的局部指针样式 |
| `feature-07` | [网页右键菜单](feature-07.md) | [PRD](../prd/feature-07.md) | 已实现 | 原生页面菜单、完整 HTML 保存和开发者工具 |
| `feature-08` | [主菜单新建与扩展管理](feature-08.md) | [PRD](../prd/feature-08.md) | 已实现 | 多窗口 IPC 路由与扩展程序管理页 |
| `feature-09` | [网页打印入口](feature-09.md) | [PRD](../prd/feature-09.md) | 已实现 | 目标网页判定与 Chromium 原生打印 |
| `feature-10` | [Chrome 风格书签与历史菜单](feature-10.md) | [PRD](../prd/feature-10.md) | 已实现 | 原生二级菜单与书签工具页 |
| `feature-11` | [历史过滤与嵌套书签文件夹](feature-11.md) | [PRD](../prd/feature-11.md) | 已实现 | 历史写入时序与书签树管理 |
| `feature-12` | [标签与书签栏溢出管理](feature-12.md) | [PRD](../prd/feature-12.md) | 已实现 | 标签压缩算法与书签溢出原生菜单 |
