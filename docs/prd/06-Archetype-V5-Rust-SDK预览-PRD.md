# Archetype V5 Rust SDK 预览产品需求文档（PRD）

| 项 | 内容 |
|----|------|
| 规范号 | 06 |
| 版本 | V5 / `archetype-sdk 0.1` 开发者预览 |
| 状态 | 实施基线 |
| 日期 | 2026-08-24 |
| 对应详设 | [06-Archetype-V5-Rust-SDK预览详设.md](../detailed-design/06-Archetype-V5-Rust-SDK预览详设.md) |

---

## 1. 产品目标

V5 将 V4 已由参考浏览器真实使用的 Runtime 边界包装为首个 UI 框架无关的 Rust SDK。目标是让合作方只依赖 `archetype-sdk`，即可启动匹配的 `archetype-runtime`、创建页面、提交静态 HTML 与有界同源资源、等待渲染结果并获得可直接呈现的 RGBA 帧。

V5 是 04 长期架构的公开预览切片，不冒充 SDK 1.0、通用浏览器内核或生产签名发行。它优先证明公共 API 形状、Runtime 生命周期和框架无关帧可以被第二个真实调用方使用。

## 2. 用户与场景

- Rust 桌面应用开发者需要在 GPUI、Iced、Slint 或自有 UI 中显示 Archetype 静态页面帧。
- SDK 集成方需要在 Runtime 崩溃时保持自身进程存活，并获得可分类的错误或断连事件。
- Archetype 维护者需要用实际 SDK 调用约束协议、类型和兼容策略，避免把内部 Browser 类型误当长期 API。

## 3. V5 必须交付

### 3.1 `archetype-sdk 0.1`

- 提供 `EngineBuilder`、`Engine`、`Page`、`PageOptions`、`StaticDocument`、`PageEvent`、`Frame` 和结构化 `SdkError`。
- Runtime 构建、页面渲染、事件等待和关闭均提供可 `.await` 的 Future，等待过程不阻塞调用方执行器线程。
- 公共 API 只暴露标准库和 Archetype 自有稳定值类型；不得暴露 GPUI、DOM、布局树、DisplayList、协议 envelope 或裸指针。
- `Frame` 公开宽、高、步长和拥有所有权的 RGBA8 字节，并能保存为 PNG。

### 3.2 Runtime 生命周期

- 支持显式 Runtime 路径和同目录自动发现；可选 SHA-256 校验在启动前完成。
- 复用 V4 一次性管道认证、握手、大小限制、背压、超时、RSS 上限和三次有限退避重启。
- Runtime 断连时完成所有等待中的 Future，并为页面投递结构化断连事件。
- `Engine::shutdown` 优雅关闭由该实例启动的 Runtime；drop 不无限等待。

### 3.3 静态页面与帧

- 合作方提交 UTF-8 HTML、视口和有界资源字节；SDK 拒绝无效 URL、零视口、超限文档和超限资源。
- Runtime 继续执行 HTML/CSS/布局/绘制，SDK 将内部 DisplayList 栅格化为公开 RGBA 帧。
- 同一页面使用单调 Navigation ID；旧结果不得覆盖新结果。
- 提供不依赖 GUI 框架的可运行示例，创建页面、渲染中英文与 Flexbox 内容并输出 PNG。

### 3.4 文档与兼容

- MSRV 保持 Rust `1.85`，SDK 版本为 `0.1.x`，SemVer `1.0` 前允许经过迁移说明的破坏性调整。
- V5 SDK 只声明兼容当前 Protocol v4 与 Runtime `0.5.x`；不伪造前一主协议兼容。
- 发布机器可读兼容矩阵、API 文档、部署说明、安全边界和恢复说明。

## 4. 验收标准

| 维度 | 验收标准 |
|------|----------|
| 接入 | 独立示例仅通过 `archetype-sdk` 公共 API 启动 Runtime、渲染页面并写出非空 PNG |
| API | rustdoc 构建无警告，公共签名扫描不含 GPUI、DOM、布局、DisplayList 或协议类型 |
| 异步 | 未就绪的构建、渲染和事件 Future 返回 `Poll::Pending`，不在 poll 中等待进程或通道 |
| 隔离 | 强制终止 Runtime 100 次，调用进程存活；请求收到成功恢复或结构化断连 |
| 帧 | 固定输入生成尺寸、步长和字节数正确的 RGBA8 帧，PNG 可解码且包含非白像素 |
| 限制 | 哈希不匹配、无效 URL、无效视口、文档/资源超限、背压和协议错误均结构化失败 |
| 兼容 | 机器矩阵准确声明 SDK `0.1.x`、Protocol v4、Runtime `0.5.x` 与 macOS 范围 |
| 质量 | 格式、严格 Clippy、全测试、rustdoc、覆盖率、许可证和现有 fuzz 闸门通过 |

## 5. 明确不进入 V5

- SDK 1.0 或跨协议主版本兼容承诺。
- JavaScript、DOM mutation、Service Worker、WebAssembly、扩展和云同步。
- Runtime 自主网络/Cookie/文件权限；V5 仍由调用方提供经过策略处理的字节。
- 共享内存或 GPU 句柄帧、音视频和多 Renderer 站点隔离。
- Windows/Linux Runtime、Developer ID 签名、公证、自动更新和商业私有 Registry 发行。

## 6. 实施顺序

| 阶段 | 交付 | 退出条件 |
|------|------|----------|
| A 规格冻结 | API、帧、发现、版本和安全 ADR | PRD/详设成对且决策无占位 |
| B SDK 客户端 | Runtime client 提取、异步 Future、Engine/Page | fake/real Runtime 生命周期测试通过 |
| C 公开帧 | RGBA 栅格、PNG、事件和旧导航过滤 | 固定帧与示例通过 |
| D 集成迁移 | 参考 Browser 使用 SDK 内部客户端 | Browser 行为与 V4 验收不回退 |
| E 发布验收 | rustdoc、兼容矩阵、双语文档和示例 | V5 全部验收项通过 |

## 7. 发布条件

V5 只有在 SDK 有真实参考 Browser 调用方和独立合作方示例、公共边界扫描与 Runtime 故障测试均通过后才能标记完成。进程拆分不得描述为系统沙箱；未签名开发构建不得描述为生产发行。

## 8. 相关文档

- [V5 Rust SDK 预览详细设计](../detailed-design/06-Archetype-V5-Rust-SDK预览详设.md)
- [Rust SDK 与 Runtime 长期 PRD](./04-Archetype-Rust-SDK与Runtime-PRD.md)
- [V4 安全运行时 PRD](./05-Archetype-V4-安全运行时-PRD.md)
