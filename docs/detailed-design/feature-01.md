# Feature 01 浏览器外壳与本地安全体验详细设计

| 字段 | 内容 |
|------|------|
| Feature ID | `feature-01` |
| 状态 | 已完成 |
| 对应 PRD | [feature-01.md](../prd/feature-01.md) |
| 主要模块 | `arch-browser::ui`、`runtime_broker`、`profile_cookies`、`arch-store` |

## 1. 总体设计

Feature 01 只调整参考浏览器外壳和 Browser-owned 安全能力。Renderer Runtime 仍不建立网络连接；页面、Cookie、favicon 和设置由 Browser 进程负责，再将有界静态文档交给 Runtime。

```text
TitleBar: [Space] [Tab][Tab][+]
Toolbar: [Back][Forward][Reload] [Address + Bookmark] [User/Settings]
Browser broker: Document -> same-origin resources -> Runtime
Profile: SQLite state + encrypted Cookie payload + environment-specific key
```

## 2. 地址栏与工具栏

- `QuickBrowser::toolbar` 不再创建清空按钮和前往箭头。
- 地址输入继续由 `InputEvent::PressEnter` 调用 `navigate_current`。
- `bookmark-current-page` 位于地址输入容器右端，打开目标文件夹菜单。
- `profile-settings` 使用用户图标，打开外观设置菜单；它不是登录入口。
- 控件均使用现有 gpui-component 按钮、图标和 tooltip，不引入新的视觉体系。

## 3. 标签页布局

- `QuickBrowser::tab_strip` 在可滚动 `tab-list` 内按顺序创建全部标签。
- 新建标签按钮作为标签列表最后一个 child，因此始终跟随最后一个标签。
- 标签宽度保持 `72..220px`，列表溢出后横向滚动。
- 标签 favicon 来自对应 `RenderedPage`，不共享错误页面或其他标签的数据。

## 4. 外观状态

`AppearancePreference` 取值为 `System`、`Light`、`Dark`：

- `BrowserCore::appearance_preference` 从 profile state 读取。
- `BrowserCore::set_appearance_preference` 写入 SQLite 状态。
- `apply_appearance` 将选择映射到 GPUI `ThemeMode`。
- 仅当选择 `System` 时订阅系统窗口外观变化；不存在按本地时间切换逻辑。

## 5. favicon 管线

1. Browser 解析页面 HTML，寻找第一个 `link`，其 `rel` token 包含 `icon`。
2. 没有显式声明时，构造同源 `/favicon.ico`。
3. 使用当前 CookieJar、1 MiB 上限和非顶层请求语义加载。
4. 校验请求 URL 与最终 URL 均同源。
5. 栅格图缩放并归一化为不超过 `32x32` 的 PNG。
6. `RenderedPage.favicon_png` 交给标签页 GPUI image 元素显示。

favicon 是可选元数据；任何失败都返回 `None`，不得让页面导航失败。

## 6. 百度与网络加载

- `QuickBrowser::start_render_request` 在后台线程创建受限 `Loader`。
- Browser broker 加载主文档、同源样式表和图片，并携带 Browser-owned Cookie 状态。
- `StaticDocument` 通过版本化协议交给 Runtime；Runtime 只消费已代理字节。
- 导航错误按 TLS、连接、HTTP 状态、超时、资源过大等类型映射到本地化错误页。
- JavaScript 和跨域静态资源继续输出诊断，不绕过 V4 安全边界。

## 7. Debug 与 Release 密钥策略

| 构建 | 密钥来源 | 行为 |
|------|----------|------|
| Debug | `<profile>.cookie-key` | 首次生成 32 字节随机密钥；Unix 权限 `0600`；不访问 Keychain |
| macOS Release | Keychain generic password | service 固定，account 由 profile 绝对路径摘要生成 |
| 非 macOS Release | 不支持持久 Cookie profile | 返回结构化错误，不降级为不安全明文密钥 |

Cookie 值始终使用 XChaCha20-Poly1305 加密并绑定固定 AAD。密钥文件不是 Cookie 明文。

## 8. 测试与证据

- 外观偏好 SQLite round-trip。
- 默认 favicon 下载、归一化和跨域拒绝。
- Cookie 加密认证、旧密钥状态丢弃和 Debug 密钥稳定性/权限。
- 地址解析、Cookie-aware broker、导航错误分类和标签邻接选择。
- 实现提交：`1c9ec90 feat(browser): refine chrome settings and navigation`、`ebba2a1 fix(profile): avoid Keychain prompts in debug builds`。

后续的 SVG favicon、压缩响应、User-Agent 分流、标签常驻和 `about:blank` 设计见 [feature-02](./feature-02.md)。
