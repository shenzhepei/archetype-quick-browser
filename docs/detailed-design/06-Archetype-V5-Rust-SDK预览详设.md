# Archetype V5 Rust SDK 预览详细设计

| 项 | 内容 |
|----|------|
| 规范号 | 06 |
| 对应 PRD | [06-Archetype-V5-Rust-SDK预览-PRD.md](../prd/06-Archetype-V5-Rust-SDK预览-PRD.md) |
| 版本 | V5 / `archetype-sdk 0.1` 开发者预览 |
| 状态 | 实施基线 |
| 日期 | 2026-08-24 |

---

## 1. 冻结决策

| ADR | V5 决策 | 后续边界 |
|-----|---------|----------|
| API 版本 | `archetype-sdk 0.1.x`，MSRV 1.85 | 1.0 前通过迁移说明演进 |
| IPC | V4 的认证 stdin/stdout 长度前缀 JSON，Protocol v4 | transport 可在新能力位后替换 |
| 帧 | IPC 内部 DisplayList；SDK 公共边界为拥有所有权的 RGBA8 | 共享内存/GPU 句柄后移 |
| 拓扑 | 合作方进程 + 单个 Runtime | 站点级 Renderer 后移 |
| 资源 | 调用方提供 HTML 与资源字节，Runtime 无网络/文件权限 | Runtime 自主 broker 后移 |
| 发现 | 显式路径优先，否则同目录 `archetype-runtime` | 不搜索 PATH，不下载二进制 |
| 完整性 | 可选调用方提供 SHA-256；缺失不声称已验证 | 签名校验进入生产发行 |
| 恢复 | 100 ms、500 ms、2 s，最多三次；只重试幂等渲染 | POST/权限决定不自动重放 |
| 平台 | macOS 未签名开发构建与现有沙箱探针 | Windows/Linux、签名、公证后移 |

## 2. Crate 与依赖

```text
Partner UI / arch-browser
        |
        v
archetype-sdk
  |- public Engine / Page / Frame / Event / Error
  |- private async bridge and rasterizer
  `- hidden internal RuntimeClient used by arch-browser
        |
        v
archetype-protocol -> archetype-runtime -> HTML/CSS/Layout/Paint
```

- 新建 `crates/archetype-sdk`，成为合作方唯一需要声明的 Archetype 依赖。
- V4 `runtime_supervisor` 移入 SDK 的隐藏内部模块；`arch-browser` 使用 `internal-runtime-client` feature 迁移调用。
- SDK 内部可以依赖 `archetype-protocol`、`archetype-types`、`arch-paint`、`cosmic-text` 和 `image`，但这些类型不得出现在文档化公共签名。
- `archetype-runtime` 的进程协议保持当前语义，避免 V5 同时重写 transport。

## 3. 公共 API

```rust
let engine = Engine::builder()
    .runtime_path("./archetype-runtime")
    .expected_runtime_sha256(hex_digest)
    .build()
    .await?;
let page = engine.create_page(PageOptions::new(1280, 800)).await?;
let navigation = page.render(StaticDocument::new(url, html)).await?;
let event = page.next_event().await;
navigation.frame().save_png("frame.png")?;
engine.shutdown().await?;
```

公开类型：

- `EngineBuilder`：Runtime 路径、可选摘要和资源限制。
- `Engine`：创建页面、状态读取和关闭；内部持有共享 client。
- `Page`：稳定 `PageId`、单调导航计数、有界事件接收端。
- `PageOptions`：`width_px`、`height_px`；范围 `1..=65535`。
- `StaticDocument`：`ArchetypeUrl`、UTF-8 HTML、`Vec<Resource>`；HTML 最大 4 MiB，单资源 4 MiB，总资源 8 MiB。
- `Frame`：`width_px`、`height_px`、`stride_bytes`、`Vec<u8>` RGBA8；验证 `len == stride * height`。
- `PageEvent`：`NavigationStarted`、`FrameReady`、`RuntimeDisconnected`、`Failed`。
- `SdkError`：`Configuration`、`Integrity`、`Protocol`、`Runtime`、`Disconnected`、`Limit`、`Io`。

所有可扩展枚举标记 `#[non_exhaustive]`。公开 API 不重导出协议或绘制 crate。

## 4. 非阻塞 Future

SDK 不绑定 Tokio 或 async-std。内部使用一次性共享状态：

```text
Pending { result: None, waker: None }
worker thread -> set result -> take waker -> wake
Future::poll -> take result or store latest waker and return Pending
```

- Runtime 启动/握手、通道等待、栅格和 PNG 编码都在工作线程执行。
- `poll` 只锁定短生命周期 mutex，不调用 `recv`、`wait`、I/O 或栅格函数。
- Future 被丢弃时工作可以完成但结果被释放；页面 Navigation ID 仍阻止旧结果进入事件队列。
- 页面事件队列容量 64；旧 `FrameReady` 可被新帧替换，`RuntimeDisconnected` 和 `Failed` 不得静默丢弃。

## 5. Runtime Client 提取

- 保留 V4 启动令牌、握手、请求 ID、5 秒超时、512 MiB RSS、64 MiB 在途字节和三次重启退避。
- `StaticDocument` 内部转换为 Protocol `RenderDocument`，完整编码后再次检查 16 MiB 帧限制。
- `RuntimeRenderedPage` 只在 SDK 隐藏模块和参考 Browser 之间使用，不属于 0.1 公共 API。
- child 退出后当前请求完成为 `Disconnected`；监督线程尝试有限重启。幂等 render 可由调用方再次提交，SDK 不重放未知状态。

## 6. RGBA 栅格

- 从 V4 snapshot renderer 提取无 GPUI 的栅格实现到 SDK 私有模块。
- 输出尺寸来自 `PageOptions`，背景固定不透明白色，格式固定 RGBA8、每像素 4 字节、stride 为 `width * 4`。
- 文本使用仓库固定 Noto Sans SC，确保合作方示例和自动化跨机器确定。
- 图片仅解码 Runtime 返回的已 broker 字节；失败图片保持 HTML alt 文本结果，不读取路径或网络。
- `Frame::save_png` 使用 `image` 编码，不改变原始 RGBA 数据。

## 7. 状态与事件

```text
Engine: Building -> Ready -> ShuttingDown -> Stopped
                     |            ^
                     -> Disconnected

Page: Created -> Rendering(n) -> Ready(n)
          |            |            |
          +----------> Failed(n) <---+
```

- 每次 `Page::render` 原子递增 Navigation ID。
- worker 完成后先比较页面当前 ID；旧结果返回 `StaleNavigation`，不发送帧事件。
- Runtime 错误映射为 SDK 自有错误码和脱敏消息，不暴露命令行、Cookie、HTML 或用户目录。
- `Engine::shutdown` 幂等；首次发送协议关闭，后续调用直接返回已停止。

## 8. 完整性与安全

- 显式 Runtime 必须是普通可执行文件；自动发现只检查 SDK 进程可执行文件同目录。
- 配置摘要时，以流式 SHA-256 读取 Runtime，使用常量时间字节比较；不把摘要校验描述为代码签名。
- SDK 不向 Runtime 发送 Cookie、密码、源文件路径或任意环境变量。
- 资源 URL 必须与文档同源；`file:` 资源不进入 V5 公共 SDK，避免公共 API 接受路径能力。
- 进程隔离、开发 sandbox probe 和生产 App Sandbox 是不同证据层级。

## 9. 兼容矩阵

机器文件 `docs/sdk-compatibility.json` 固定以下首行：

```json
{"sdk":"0.1.x","protocol_major":4,"runtime":"0.5.x","target":"aarch64-apple-darwin","status":"developer_preview"}
```

SDK 构建时版本为 `0.1.0`，工作区其他 crate 为 `0.5.0`。V5 不测试不存在的 Protocol v3 兼容，也不承诺 SDK 0.1 与未来协议主版本兼容。

## 10. 测试与验收

- Future：首次 poll 为 Pending、worker 唤醒、取消和并发完成。
- Builder：显式/同目录发现、无文件、不可执行、摘要匹配与不匹配。
- Page：ID 唯一、Navigation ID 单调、旧结果过滤、队列上限和断连事件。
- Frame：尺寸、stride、字节长度、非白像素、PNG round-trip 和中英文固定字体。
- Runtime：真实握手、渲染、优雅关闭、100 次 kill/restart、RSS/字节/超时限制。
- API：`cargo doc -p archetype-sdk --no-deps` 严格通过，生成 rustdoc JSON/源码扫描不含禁止类型。
- 示例：`cargo run -p archetype-sdk --example partner_render -- <runtime> <output.png>` 成功。

## 11. 实施切片

| 切片 | 代码范围 | 验证 |
|------|----------|------|
| A1 规格 | 06 PRD、详设和索引 | 链接与编号检查 |
| B1 client | supervisor 提取与 Browser 迁移 | V4 Runtime/Browser 回归 |
| B2 async API | Future、Builder、Engine、Page、错误 | fake 与 real Runtime 测试 |
| C1 frame | 可变尺寸栅格、RGBA、PNG | frame 单元与金样 |
| C2 events | 导航过滤、有界事件、断连 | 并发与故障注入 |
| D1 示例 | partner render 与 API 文档 | 独立 PNG 验收 |
| E1 发布 | 兼容矩阵、README、许可证、版本 | 全工作区和 V5 验收 |

## 12. 完成定义

V5 完成必须满足 06 PRD 全部验收项，同时不降低 V4 Browser、Runtime、沙箱、表单、Flexbox、休眠和发行证据。SDK 0.1 只能标记开发者预览，不得宣称 04 长期架构、生产签名或 JavaScript 已完成。

## 13. 相关文档

- [V5 Rust SDK 预览 PRD](../prd/06-Archetype-V5-Rust-SDK预览-PRD.md)
- [Rust SDK 与 Runtime 长期详设](./04-Archetype-Rust-SDK与Runtime详设.md)
- [V4 安全运行时详设](./05-Archetype-V4-安全运行时详设.md)
