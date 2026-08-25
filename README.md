# Archetype Quick Browser

<!-- repo-languages:start -->
English | [简体中文](README-zh-CN.md)
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

Archetype is a focused desktop browser whose webpages run in Electron's bundled Chromium. React,
TypeScript, Vite, and SCSS implement the Browser Chrome; websites remain isolated in sandboxed
`WebContentsView` instances with Chromium JavaScript, Web APIs, networking, and site storage.

[Browser Chrome preview](https://shenzhepei.github.io/archetype-quick-browser/)

![Archetype browser preview](docs/preview.webp)

## Features

- Independent Chromium tabs with preserved state, title, favicon, and loading feedback.
- Address search, back, forward, reload, stop, bookmark, and managed popup workflows.
- Persistent bookmarks, browsing history, tabs, theme, and language preferences.
- Chromium-managed Cookies, cache, localStorage, IndexedDB, and Service Workers.
- Native `archetype://history` and `archetype://settings/*` views inside the Browser Chrome.
- System, light, and dark appearance modes plus English and Simplified Chinese UI.

## Development

Install Node.js 24 or newer and enable Corepack:

```bash
corepack enable
pnpm install
pnpm dev
```

`pnpm dev` is the startup command. It opens the Electron application; a normal web browser can only
show the static Browser Chrome preview and cannot host Electron `WebContentsView` pages.

Build and test with:

```bash
pnpm typecheck
pnpm test:coverage
pnpm build
```

Create unsigned macOS DMG and ZIP artifacts with `pnpm package:mac`. Create Windows NSIS and ZIP
artifacts with `pnpm package:win` on Windows or a host with the required Wine tooling.

## Architecture

| Module | Responsibility |
| --- | --- |
| `src/main` | Electron window, Chromium tabs, navigation, session, persistence, and IPC handlers |
| `src/preload` | Typed, allowlisted bridge exposed only to the Browser Chrome |
| `src/renderer` | React Browser Chrome, internal pages, themes, and runtime localization |
| `src/shared` | Process-neutral state and command contracts |

The [PRD](docs/prd/01-Archetype-Chromium浏览器-PRD.md) and
[detailed design](docs/detailed-design/01-Archetype-Chromium浏览器详设.md) define the current scope and
remaining release work. The prior Rust HTML/CSS renderer has been removed rather than retained as a
fallback.

## Status

The Chromium architecture and core browser workflows are implemented. Permissions currently default
to deny. Downloads UI, certificate-error UX, crash recovery, code signing, notarization, auto-update,
and Linux packages remain before a production release.

## License

Licensed under the [Apache License 2.0](LICENSE).
