# Feature 03 GitHub Release 版本检查 PRD

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-03` |
| 状态 | 已实现 |
| 对应详设 | [feature-03.md](../detailed-design/feature-03.md) |
| 所属规范 | [01 Archetype Chromium 浏览器](01-Archetype-Chromium浏览器-PRD.md) |

## 1. 背景

“关于 Archetype”中的版本号当前写死为 `0.1.0`，无法反映实际安装包版本，也不能判断 GitHub Release 是否存在更新。

## 2. 需求与验收

| ID | 需求 | 验收标准 |
| --- | --- | --- |
| F03-01 | 当前版本 | 关于页显示 Electron 安装包 `app.getVersion()`，与打包使用的 `package.json` 版本一致 |
| F03-02 | 最新版本 | 主进程查询 `shenzhepei/archetype-quick-browser` 的 GitHub latest Release，并读取 tag 与 Release URL |
| F03-03 | 版本判断 | 能区分已是最新版、发现新版、尚无 Release、网络或响应异常四种状态 |
| F03-04 | 更新入口 | 发现新版时可通过明确按钮在系统浏览器打开对应 GitHub Release 页面 |
| F03-05 | 重试 | 关于页提供重新检查命令，加载期间按钮不可重复触发 |
| F03-06 | 国际化 | 所有状态、版本标签和命令提供英文与简体中文文案，切换语言不改变布局高度 |
| F03-07 | 安全 | Renderer 不直接访问网络，不可传入任意更新地址；主进程只允许打开经过仓库前缀校验的 GitHub Release URL |

## 3. 非目标

- 本 Feature 不自动下载、安装或重启应用。
- 不处理 nightly、草稿 Release 或 GitHub prerelease；latest endpoint 只判断正式 Release。
