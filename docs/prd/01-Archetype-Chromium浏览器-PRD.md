# 01 Archetype Chromium 浏览器 PRD

| 字段 | 内容 |
| --- | --- |
| 规范号 | `01` |
| 状态 | 实施中 |
| 产品 | Archetype Quick Browser |
| 对应详设 | [01-Archetype-Chromium浏览器详设.md](../detailed-design/01-Archetype-Chromium浏览器详设.md) |

## 1. 背景

旧仓库使用 Rust 自研 HTML/CSS 渲染链，无法在合理周期内覆盖 JavaScript、Web API 和现代网页兼容性。本次重建删除旧渲染链，以 Electron 内置 Chromium 作为唯一普通网页内核，React 只实现 Browser Chrome。

## 2. 目标

| ID | 需求 | 验收标准 |
| --- | --- | --- |
| R01-01 | Chromium 网页内核 | `http`、`https`、`file`、`about:blank` 均由 Electron `WebContentsView` 渲染，JavaScript 与 Chromium Web API 默认可用 |
| R01-02 | 多标签 | 可新建、选择和关闭标签；新增按钮紧跟最后一个标签；切换标签不重新加载页面 |
| R01-03 | 导航 | 地址栏支持 URL、域名和搜索词；支持后退、前进、刷新、停止及回车导航 |
| R01-04 | 页面状态 | 标签显示标题、favicon 和加载状态；无 favicon 时显示默认图标 |
| R01-05 | 收藏与历史 | 地址栏右侧提供收藏按钮；收藏与历史重启后恢复；三点菜单提供历史入口 |
| R01-06 | 设置 | 头像和三点菜单可打开 `archetype://settings/appearance`；支持跟随系统、浅色、深色 |
| R01-07 | 内部页 | 提供 `archetype://history`、`archetype://settings/appearance`、`archetype://settings/about` |
| R01-08 | 国际化 | 首次启动默认英文，可切换简体中文并持久化 |
| R01-09 | Chromium 站点数据 | Cookie、cache、localStorage、IndexedDB 等由持久 Electron session 管理，不写入产品 JSON |
| R01-10 | 分发 | macOS 可生成 DMG/ZIP；仓库提供可重复的 Node 24 + pnpm 构建、测试与覆盖率工作流 |

## 3. 安全与隐私

- 网页 `webContents` 启用 sandbox、context isolation、web security，禁用 Node integration。
- Browser Chrome 只通过白名单 preload IPC 调用主进程，不把 Electron API 暴露给网页。
- 网站弹窗转成受管标签页，不创建脱离 Browser Chrome 的窗口。
- 第一版默认拒绝网站权限请求；权限 UI 作为后续功能。
- 浏览数据只保存在 Electron userData 目录，不采集或上传遥测。

## 4. 非目标

- 不实现 Chrome 扩展商店、Google 账号同步、DRM、无痕窗口和跨设备同步。
- 不自行解析或渲染 HTML/CSS，不保留旧 Rust renderer fallback。
- 第一版不承诺 Windows/Linux 安装包、自动更新、签名与公证。

## 5. 完成条件

核心标签、导航、状态同步、收藏、历史、设置、持久化与打包通过自动化和真实 Electron 冒烟验证后，本规范可标记完成。
