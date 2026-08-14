# Archetype Rust SDK 与 Runtime 产品需求文档（PRD）

| 项 | 内容 |
|----|------|
| 规范号 | 04 |
| 版本 | SDK/Runtime 长期架构 |
| 状态 | 规划基线 |
| 目标合作方 | 使用 Rust 开发桌面浏览器的合作方 |
| 日期 | 2026-08-14 |
| 对应详设 | [04-Archetype-Rust-SDK与Runtime详设.md](../detailed-design/04-Archetype-Rust-SDK与Runtime详设.md) |

---

## 1. 产品目标

Archetype 向使用 Rust 的合作方提供 UI 框架无关的浏览器内核能力。合作方只直接依赖公开的 `archetype-sdk` crate，可自行选择 GPUI、Iced、Slint 或其他 Rust UI 技术；内核在独立的 `archetype-runtime` 进程中运行，通过版本化 IPC 与 SDK 通信。

该交付模式同时解决三个问题：避免依赖不稳定的 Rust 动态库 ABI、隔离网页主动内容与合作方 UI、允许 Runtime 与合作方应用按明确兼容策略独立升级。

## 2. 用户与场景

- 合作方用 Rust 构建自有桌面浏览器、阅读器或垂直 Web 容器。
- 合作方需要控制窗口、标签页、书签和品牌体验，但不直接访问内核内部对象。
- Archetype 需要保护闭源内核实现，并能单独签名、更新和诊断 Runtime。

## 3. 必须交付

### 3.1 Rust SDK

- 提供异步 `Engine`、`Page`、命令、事件和生命周期 API。
- 公开 API 只使用 Archetype 自有、可版本化的稳定值类型。
- SDK 不暴露 GPUI、Iced、Slint、V8 Handle、DOM 引用、裸指针或内部布局结构。
- 提供 Runtime 启动、连接、版本协商、崩溃检测、重连和优雅关闭能力。
- 提供可运行的最小合作方示例和 API 文档。

### 3.2 独立 Runtime

- Runtime 作为独立签名进程交付，不采用 Rust `dylib` 作为跨版本调用边界。
- 承载网络、HTML/CSS/DOM、JavaScript、布局、绘制和 Renderer 隔离编排。
- 对请求设置超时、资源上限、背压和可取消边界。
- Runtime 崩溃或被终止时，不得带崩合作方 UI 进程。

### 3.3 版本化协议

- SDK 与 Runtime 建立连接时必须交换 SDK、协议、Runtime 和能力版本。
- 命令、响应和事件带请求 ID、Page ID 及必要的导航序号。
- 未知可选字段可忽略；不兼容的主版本必须在执行命令前明确失败。
- 协议必须限制消息长度、句柄数量、共享内存大小和事件队列深度。

### 3.4 交付方式

- 默认商业交付为“公开 SDK 源码 + 签名 Runtime 二进制”。
- Runtime 按目标平台和架构独立构建、签名并提供校验信息。
- 私有 Cargo Registry 可用于 SDK 分发，但不得宣称其能阻止合作方读取 crate 源码。

## 4. 验收标准

| 维度 | 验收标准 |
|------|----------|
| 接入 | 新合作方仅依赖公开 SDK 和 Runtime，能在示例 UI 中创建页面、导航并展示帧 |
| 隔离 | 强制终止 Runtime 后合作方 UI 保持存活，并收到结构化断连事件 |
| 兼容 | SDK 1.x 可与声明兼容的 Protocol v3 / Runtime 3.x 完成协商 |
| 工具无关 | 公共 API 和协议中不存在任何具体 Rust UI 框架类型 |
| 安全 | 畸形、超限或未授权 IPC 消息被拒绝且不会导致任一进程崩溃 |
| 升级 | Runtime 可在不迁移合作方内部 UI 对象的情况下独立替换 |
| 文档 | API、兼容矩阵、部署、签名、崩溃恢复和安全限制均有可执行示例 |

## 5. 非目标

- 不提供 C ABI、Qt、Swift、C#、JNI 或 WebAssembly 绑定。
- 不承诺 Rust 编译器内部 ABI 或 Rust `dylib` 跨版本兼容。
- 不把 GPUI 作为公开内核接口的一部分。
- 不允许合作方获得 V8 Handle、Renderer 内存或可变 DOM 指针。
- 独立进程不自动等同于系统沙箱；各平台沙箱属于 Runtime 发布条件。
- 04 不表示 V3 已经具备 V8、多进程 Renderer 或操作系统沙箱。

## 6. 进入实施前必须决策

1. 帧传输采用序列化 DisplayList、共享内存位图还是平台 GPU 句柄。
2. IPC 编码、传输层、能力协商和兼容窗口。
3. Browser、Network、Renderer、GPU 的首版进程拓扑。
4. macOS、Windows、Linux 的沙箱策略、代码签名与子进程启动规则。
5. Runtime 二进制发现、校验、更新、回滚和崩溃收集策略。
6. SDK 的最低 Rust 版本、SemVer 承诺以及第三方类型暴露规则。

每项决策必须以 ADR 冻结；在帧传输和进程拓扑未冻结前，不启动公开 SDK 兼容承诺。

## 7. 与 V3 的关系

V3 继续使用进程内 GPUI 参考浏览器完成静态渲染垂直切片。V3 需要保持 UI 与引擎间的命令/事件边界，使用稳定 ID 和可序列化值对象，为后续拆进程准备，但不提前创建只有空接口的 SDK/Runtime crate。

## 8. 相关文档

- [Rust SDK 与 Runtime 详细设计](../detailed-design/04-Archetype-Rust-SDK与Runtime详设.md)
- [V3 PRD](./03-Archetype-V3-PRD.md)
- [总体 PRD](./01-Archetype-PRD.md)

