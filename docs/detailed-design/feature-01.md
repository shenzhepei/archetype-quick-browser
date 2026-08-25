# Feature 01 跨平台窗口标题栏间距详细设计

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-01` |
| 状态 | 已实现 |
| 对应 PRD | [feature-01.md](../prd/feature-01.md) |
| 主要模块 | `src/main/index.ts`、`src/preload/index.ts`、`src/renderer/styles/main.scss` |

## 1. 平台窗口策略

- macOS 使用 `titleBarStyle: hiddenInset`，traffic lights 位于 `(16, 16)`。
- Windows 使用 `titleBarStyle: hidden` 和高度 `40px` 的 `titleBarOverlay`，保留系统窗口控制按钮。
- Linux 不覆盖标题栏参数，由窗口管理器绘制原生标题栏。

主进程在恢复设置后同步 `nativeTheme.themeSource`。Windows 上通过 `setTitleBarOverlay` 设置浅色或深色背景与图标色，并监听系统主题更新。

## 2. 渲染层安全区

preload 只暴露枚举化平台值，渲染入口写入 `html[data-platform]`。标签栏按平台应用稳定安全区：

| 平台 | 左侧安全区 | 右侧安全区 |
| --- | ---: | ---: |
| macOS | `88px`，窄窗口 `82px` | `42px` 拖拽区 |
| Windows | `12px` | `144px` 窗口控制区 |
| Linux/Web preview | `12px` | `42px` 拖拽区 |

标签列表保持 `overflow-x: auto`，安全区不参与横向滚动，所有交互元素继续使用 `-webkit-app-region: no-drag`。

## 3. Windows 构建

`pnpm package:win` 先执行完整构建，再由 electron-builder 生成 NSIS 和 ZIP。`build/icon.png` 从现有品牌图标导出为 `1024x1024` PNG。非 Windows 主机需要 Wine 等 electron-builder 依赖。

## 4. 验证

- `pnpm typecheck`
- `pnpm test`
- `pnpm build`
- macOS 真实 Electron 窗口检查 traffic lights、首个标签和新增标签按钮
- Windows 安装包与原生三键行为需在 Windows 或配置完整的交叉构建环境中验证
