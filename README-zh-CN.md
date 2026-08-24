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

![Archetype 确定性渲染预览](fixtures/screenshots/07-box-paint.png)

_来自仓库内 V3 固定测试语料的 Archetype 确定性渲染预览。_

## 功能

- 桌面标签页、空间、嵌套书签和持久化导航历史。
- HTML5 解析以及初步的 CSS 解析、层叠、样式、布局和绘制流水线。
- 受限的本地与 HTTP(S) 资源加载，并提供 TLS、解析和渲染分类错误页面。
- PNG 和 JPEG 解码、递归块布局、行内文本、边框与背景。
- 基于 SQLite 的浏览器状态持久化，并支持损坏配置恢复。
- 可取消的后台导航、各标签页独立的渲染页面，以及当前标签自动保持可见的溢出滚动。

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

应用会将换行分隔的 JSON 诊断日志写入
`<应用支持目录>/Archetype/logs/archetype.jsonl`。日志仅保存在本地，应用不会收集或上传遥测。
开发或测试时可设置 `ARCHETYPE_DATA_DIR`，将配置与日志隔离到指定目录。

## 验证

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

工作区测试会将全部 30 个金样渲染结果与仓库内固定的 `1280x800` PNG 参考图比较。
确认渲染变化符合预期后，可使用以下命令重新生成并审查参考图：

```bash
cargo run -p arch-browser --example update_snapshots
```

重启恢复集成测试会强制终止独立的浏览器配置进程，再验证空间、分层书签、标签页、标题、顺序和
选中项均可恢复。CI 会对 HTML、CSS 和版本化协议执行模糊测试。如需在本地复现，请安装 Rust nightly
和 [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz)，然后运行：

```bash
cargo +nightly fuzz run html -- -max_total_time=20 -timeout=5
cargo +nightly fuzz run css -- -max_total_time=20 -timeout=5
cargo +nightly fuzz run protocol -- -max_total_time=20 -timeout=5
```

如需复现 V3 的启动、性能、内存和一分钟资源趋势证据，请构建全部 release 二进制并运行：

```bash
cargo build --release --bins
./target/release/arch-v3-acceptance \
  --duration-seconds 60 \
  --cycle-delay-milliseconds 1000 \
  --startup-samples 20 \
  --output docs/v3-acceptance-report.json
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
| `archetype-types`、`archetype-protocol` | 稳定值对象、分帧 IPC、协商、路由和有界 transport |
| `archetype-runtime` | 静态文档 Renderer 子进程与分帧命令循环 |
| `arch-browser` | 桌面外壳、编排、本地化和渲染集成 |
| `arch-html`、`arch-dom` | HTML 解析和文档表示 |
| `arch-css`、`arch-style` | CSS 解析、层叠和计算样式 |
| `arch-layout`、`arch-paint` | 布局和显示列表生成 |
| `arch-net` | 受限的文档与资源加载 |
| `arch-session`、`arch-store` | 导航状态和 SQLite 持久化 |

范围明确的产品需求和详细实现方案位于：

- [`docs/prd/03-Archetype-V3-PRD.md`](docs/prd/03-Archetype-V3-PRD.md)
- [`docs/detailed-design/03-Archetype-V3-详设.md`](docs/detailed-design/03-Archetype-V3-详设.md)
- [`docs/prd/05-Archetype-V4-安全运行时-PRD.md`](docs/prd/05-Archetype-V4-安全运行时-PRD.md)
- [`docs/detailed-design/05-Archetype-V4-安全运行时详设.md`](docs/detailed-design/05-Archetype-V4-安全运行时详设.md)
- [`docs/v3-acceptance.md`](docs/v3-acceptance.md) 及其
  [机器可读报告](docs/v3-acceptance-report.json)

## V3 状态

- V3 已完成：工作区和 CI；带标题栏标签页、紧凑型空间切换和按空间管理根书签的桌面外壳；
  DOM 与 HTML 解析；初步的 CSS 解析器和层叠；递归块盒与行内文本；支持定位文本、图片、
  背景和边框的可序列化显示列表；文本颜色、字体族、字重、样式、对齐、行高、空白处理和
  溢出裁剪；受限的文档、样式表、PNG 与 JPEG 加载和图片回退；分类错误页面；支持损坏配置恢复的 SQLite
  空间与页面持久化；不含遥测的本地结构化 JSONL 诊断日志；
  全局标签页持久化、分层空间书签存储和根书签栏、导航标识、重定向、
  链接与历史；30 个确定性测试页面全部完成，并通过全语料断言验证标题、绘制文本、
  解析后的链接、已加载图片和预期诊断；固定 PNG 截图回归达到 V3 的 0.5% 像素差异阈值。
  强制退出后的配置恢复、HTML/CSS 解析器入口的 CI 模糊测试、可取消后台导航、标签页独立
  渲染状态，以及已记录的启动、页面流水线、帧、CPU 和 RSS 趋势证据也已完成。
- 每项验收要求的证据和已知限制维护在 [`docs/v3-acceptance.md`](docs/v3-acceptance.md)。
  JavaScript、表单、媒体、Flexbox、Grid、多进程渲染、沙箱和公开签名分发仍不属于 V3 范围。

## V4 开发进度

- 稳定类型、协议原型、C1 子进程、C2 资源 broker 与 D1 macOS 沙箱切片已经完成：UUID V7 页面 ID、单调导航 ID、经过验证的 URL
  值对象、版本化长度前缀 JSON codec、能力协商、有界内存 transport、请求路由、取消、超时、背压、
  协议模糊测试，以及真实 Renderer Runtime 握手、静态文档渲染、DisplayList 返回、异步 Browser
  监督、结构化断连和 20 轮子进程终止测试。Browser 现在传递有大小限制的同源样式表和图片字节，
  Runtime 不接收路径、不使用 `arch-net`，只解析和解码收到的字节。
- 本地文档子资源被限制在文档目录树内；跨源资源、跨源重定向会被拒绝；完整编码请求进入子进程前会按
  16 MiB 协议帧上限预检。
- Browser 使用不进入参数、环境变量、日志或协议负载的一次性令牌，通过继承管道认证每个 Runtime。
  监督器执行 5 秒请求超时、512 MiB RSS 上限和 64 MiB 在途请求预算。
- macOS CI 证明开发态 Runtime profile 会拒绝任意文件读取、loopback 与外网连接、监听端口创建和
  子进程启动；同时对 Browser 与 Runtime 测试副本进行临时签名，核验嵌入 entitlement 白名单，
  并拒绝 Runtime 获得文件或网络权限。
- 子进程路径尚未成为桌面应用默认导航路径，也不是可发布的生产签名应用包。下一切片是 E1 会话 Cookie
  与表单。只有桌面集成、可分发签名与公证、会话能力、Flexbox、休眠和发布验收全部满足配对规范后，
  V4 才算完成。

## 许可证

本项目采用 [Apache License 2.0](LICENSE) 许可。第三方归属信息见
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
