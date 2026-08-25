# Archetype Quick Browser

<!-- repo-languages:start -->
[English](README.md) | 简体中文
<!-- repo-languages:end -->

<!-- repo-badges:start -->
[![Node.js 24](https://img.shields.io/badge/Node.js-24-339933?style=flat-square&logo=nodedotjs&logoColor=white)](https://nodejs.org)
[![pnpm 10.33.2](https://img.shields.io/badge/pnpm-10.33.2-F69220?style=flat-square&logo=pnpm&logoColor=white)](https://pnpm.io)
[![React 19.2.8](https://img.shields.io/badge/React-19.2.8-61DAFB?style=flat-square&logo=react&logoColor=white)](https://react.dev)
[![Vite 7.3.6](https://img.shields.io/badge/Vite-7.3.6-646CFF?style=flat-square&logo=vite&logoColor=white)](https://vite.dev)
[![TypeScript 6.0.3](https://img.shields.io/badge/TypeScript-6.0.3-3178C6?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org)
[![Sass 1.103.1](https://img.shields.io/badge/Sass-1.103.1-CC6699?style=flat-square&logo=sass&logoColor=white)](https://sass-lang.com)
[![Test Coverage](https://img.shields.io/codecov/c/github/shenzhepei/archetype-quick-browser?style=flat-square&logo=codecov)](https://codecov.io/gh/shenzhepei/archetype-quick-browser)
[![License](https://img.shields.io/github/license/shenzhepei/archetype-quick-browser?style=flat-square)](https://github.com/shenzhepei/archetype-quick-browser/blob/HEAD/LICENSE)
[![Sponsor](https://img.shields.io/github/sponsors/shenzhepei?style=flat-square&logo=githubsponsors&label=Sponsor)](https://github.com/sponsors/shenzhepei)
<!-- repo-badges:end -->

Archetype 是一个专注的桌面浏览器，网页由 Electron 内置 Chromium 运行。React、TypeScript、
Vite 和 SCSS 实现 Browser Chrome；网站隔离在启用 sandbox 的 `WebContentsView` 中，直接使用
Chromium 的 JavaScript、Web API、网络和站点存储。

[Browser Chrome 在线预览](https://shenzhepei.github.io/archetype-quick-browser/)

![Archetype 浏览器预览](docs/preview.webp)

## 功能

- 独立 Chromium 标签页，切换时保留状态，并同步标题、favicon 和加载反馈。
- 地址搜索、后退、前进、刷新、停止、收藏和受管理的弹窗标签页。
- 持久化收藏、浏览历史、标签页、主题和语言偏好。
- 由 Chromium 管理 Cookie、cache、localStorage、IndexedDB 和 Service Worker。
- Browser Chrome 内原生实现 `archetype://history` 和 `archetype://settings/*` 页面。
- 跟随系统、浅色、深色外观，以及英文和简体中文界面。

## 开发

安装 Node.js 24 或更高版本并启用 Corepack：

```bash
corepack enable
pnpm install
pnpm dev
```

现在的启动命令是 `pnpm dev`。它会打开 Electron 应用；普通 Web 浏览器只能展示静态 Browser
Chrome 预览，无法承载 Electron `WebContentsView` 网页。

构建与测试：

```bash
pnpm typecheck
pnpm test:coverage
pnpm build
```

使用 `pnpm package:mac` 生成未签名的 macOS DMG 和 ZIP。在 Windows 或具备 Wine 工具链的
主机上，使用 `pnpm package:win` 生成 NSIS 安装包和 ZIP。

## 架构

| 模块 | 职责 |
| --- | --- |
| `src/main` | Electron 窗口、Chromium 标签、导航、session、持久化和 IPC handler |
| `src/preload` | 仅向 Browser Chrome 暴露的类型化白名单桥接层 |
| `src/renderer` | React Browser Chrome、内部页、主题和运行时国际化 |
| `src/shared` | 与进程无关的状态和命令契约 |

当前范围和后续发布工作见 [PRD](docs/prd/01-Archetype-Chromium浏览器-PRD.md)与
[详细设计](docs/detailed-design/01-Archetype-Chromium浏览器详设.md)。旧 Rust HTML/CSS renderer
已被删除，不再作为 fallback 保留。

## 状态

Chromium 架构和核心浏览器流程已经实现。网站权限目前默认拒绝。下载 UI、证书错误体验、崩溃
恢复、代码签名、公证、自动更新及 Linux 安装包仍是生产发布前的待办项。

## 许可证

本项目采用 [Apache License 2.0](LICENSE) 许可。
