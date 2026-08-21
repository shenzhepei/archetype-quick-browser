# Archetype Quick Browser

<!-- repo-languages:start -->
[English](README.md) | 简体中文
<!-- repo-languages:end -->

[![Rust](https://img.shields.io/badge/Rust-1.85%2B-000000?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![GPUI](https://img.shields.io/badge/GPUI-0.2.2-2F80ED?style=flat-square)](https://www.gpui.rs/)
[![CI](https://img.shields.io/github/actions/workflow/status/shenzhepei/archetype-quick-browser/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/shenzhepei/archetype-quick-browser/actions/workflows/ci.yml)

<!-- repo-badges:start -->
[![Test Coverage](https://img.shields.io/codecov/c/github/shenzhepei/archetype-quick-browser?style=flat-square&logo=codecov)](https://codecov.io/gh/shenzhepei/archetype-quick-browser)
[![License](https://img.shields.io/github/license/shenzhepei/archetype-quick-browser?style=flat-square)](https://github.com/shenzhepei/archetype-quick-browser/blob/HEAD/LICENSE)
[![Sponsor](https://img.shields.io/github/sponsors/shenzhepei?style=flat-square&logo=githubsponsors&label=Sponsor)](https://github.com/sponsors/shenzhepei)
<!-- repo-badges:end -->

Archetype Quick Browser 是一个面向静态 HTML 文档的开发者预览版桌面浏览器。其 Rust
工作区提供基于 GPUI/gpui-component 的桌面外壳、标题栏标签页、紧凑型空间、按空间管理的
书签、导航历史、受限的本地与 HTTP(S) 加载、HTML/CSS 处理、基础布局以及 PNG/JPEG 显示。

> [!NOTE]
> 本项目是用于开发和实验的浏览器引擎原型，并非生产级 Web 浏览器。当前桌面应用和 CI
> 以 macOS 为目标平台。

## 功能

- 桌面标签页、空间、嵌套书签和持久化导航历史。
- HTML5 解析以及初步的 CSS 解析、层叠、样式、布局和绘制流水线。
- 受限的本地与 HTTP(S) 资源加载，并提供分类错误页面。
- PNG 和 JPEG 解码、递归块布局、行内文本、边框与背景。
- 基于 SQLite 的浏览器状态持久化，并支持损坏配置恢复。

## 运行

安装 `rust-toolchain.toml` 声明的 Rust 工具链，然后运行：

```bash
cargo run -p arch-browser
```

桌面应用会将配置存储在平台的应用支持目录中。新页面默认打开第一个确定性测试页面。
桌面外壳跟随操作系统语言：中文语言环境（`zh*`）使用中文，其余语言环境回退到英文。
如需在不打开窗口的情况下通过渲染流水线检查页面，请运行：

```bash
cargo run -p arch-browser -- --inspect fixtures/pages/05-image/index.html
```

## 验证

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

如需生成与 CI 上传内容相同的 LCOV 报告，请安装
[`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) 并运行：

```bash
mkdir -p coverage
cargo llvm-cov --workspace --lcov --output-path coverage/lcov.info
```

## 架构

工作区按浏览器职责拆分为多个专用 crate：

| Crate | 职责 |
| --- | --- |
| `arch-browser` | 桌面外壳、编排、本地化和渲染集成 |
| `arch-html`、`arch-dom` | HTML 解析和文档表示 |
| `arch-css`、`arch-style` | CSS 解析、层叠和计算样式 |
| `arch-layout`、`arch-paint` | 布局和显示列表生成 |
| `arch-net` | 受限的文档与资源加载 |
| `arch-session`、`arch-store` | 导航状态和 SQLite 持久化 |

范围明确的产品需求和详细实现方案位于：

- [`docs/prd/03-Archetype-V3-PRD.md`](docs/prd/03-Archetype-V3-PRD.md)
- [`docs/detailed-design/03-Archetype-V3-详设.md`](docs/detailed-design/03-Archetype-V3-详设.md)

## 当前覆盖范围

- 已实现：工作区和 CI；带标题栏标签页、紧凑型空间切换和按空间管理根书签的桌面外壳；
  DOM 与 HTML 解析；初步的 CSS 解析器和层叠；递归块盒与行内文本；支持定位文本、图片、
  背景和边框的可序列化显示列表；文本颜色、字体族、字重、样式、对齐、行高、空白处理和
  溢出裁剪；受限的文档、样式表、PNG 与 JPEG 加载和图片回退；分类错误页面；支持损坏配置恢复的 SQLite
  空间与页面持久化；全局标签页持久化、分层空间书签存储和根书签栏、导航标识、重定向、
  链接与历史；带有全语料渲染测试的确定性测试页面。
- 待实现：完善 CSS/布局支持矩阵，将语料扩展至 30 个测试页面，改进字体塑形与链接交互，
  增加截图回归和模糊测试，记录性能基线并收集发布验收证据。

## 许可证

本项目采用 [Apache License 2.0](LICENSE) 许可。第三方归属信息见
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
