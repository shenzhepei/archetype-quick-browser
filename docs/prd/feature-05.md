# Feature 05 Chromium 内核迁移 PRD

| 字段 | 内容 |
|------|------|
| Feature ID | `feature-05` |
| 状态 | 实施中 |
| 来源 | 放弃自研 JavaScript/WebAPI 和高级 CSS 路线，网页内容统一改用 Chromium 内核 |
| 对应详设 | [feature-05.md](../detailed-design/feature-05.md) |

## 1. 决策

Archetype 不再自行追赶 JavaScript、WebAPI、HTML/CSS、完整选择器、Grid、fixed/sticky、transform、transition、animation、gradient、background-image 和复杂 stacking context。网页内容改由 Chromium Embedded Framework（CEF）渲染，GPUI 继续负责标签栏、地址栏、书签、设置、历史和其他 Browser Chrome。

原 `arch-html`、`arch-css`、`arch-style`、`arch-layout`、`arch-paint` 与 `archetype-runtime` 静态渲染链进入 legacy 状态；迁移验收完成前仅作为开发回退和旧测试基线，不再新增网页兼容属性。迁移完成后删除产品运行路径中的自研渲染器。

## 2. 功能需求

| ID | 需求 | 验收标准 |
|----|------|----------|
| F05-01 | 使用 Chromium 内核 | 普通 `http/https/file/about:blank` 标签由固定版本 CEF 创建和渲染，不再进入自研 HTML/CSS Runtime |
| F05-02 | 支持现代 Web 平台 | JavaScript、DOM/WebAPI 和 Chromium 支持的 HTML/CSS 默认启用；不再输出自研引擎 unsupported CSS/JavaScript 诊断 |
| F05-03 | 嵌入现有 GPUI 外壳 | Chromium 内容视图只占页面内容区，不覆盖标签栏、地址栏、菜单、书签栏和内部页 UI |
| F05-04 | 标签生命周期 | 每个普通标签对应独立 CEF Browser；切换只显示选中实例，关闭释放实例，休眠策略不丢失未提交页面状态 |
| F05-05 | 导航双向同步 | 地址栏、前进、后退、刷新、停止驱动 CEF；CEF 的 URL、标题、favicon、loading 和导航结果回写现有标签状态与历史 |
| F05-06 | Chromium profile | Cookie、HTTP cache、localStorage、IndexedDB、权限和站点数据由隔离的 Chromium profile 管理；Archetype SQLite 只保存产品元数据和浏览历史 |
| F05-07 | 内部协议隔离 | `archetype://history` 与 `archetype://settings/*` 保持 Browser 进程白名单内部页，不可被普通网站导航、脚本或 fetch 读取 |
| F05-08 | 多进程与沙箱 | 使用 CEF Browser/Renderer/GPU/Utility 多进程模型和平台沙箱，不使用 `--no-sandbox` 作为发布配置 |
| F05-09 | 打包运行 | macOS `.app` 包含固定版本 Chromium Framework、Resources、Locales 和 Helper app；签名顺序与 entitlements 可重复验证 |
| F05-10 | 故障可诊断 | CEF 缺失、版本不匹配、Helper 启动失败和 Renderer 崩溃显示明确错误，不静默回退成错误网页 |

## 3. 数据与兼容决策

- CEF cache/profile 根目录使用 Archetype profile 下独立 `chromium/` 目录。
- 不把 Chromium Cookie 复制到现有 SQLite；历史和书签继续由 Archetype Store 保存。
- 旧 profile 首次迁移不导入自研 Cookie，避免两套格式和加密语义混用；后续导入需要单独 Feature。
- CEF 版本固定，不使用系统 Chrome，不依赖用户已安装浏览器。
- 第一交付平台为 Apple Silicon macOS；其他平台必须各自完成 Framework/Helper/沙箱和 CI 后才能声明支持。

## 4. 验收

- 动态 JavaScript 页面、Element Plus、百度及 CSS 综合夹具可交互呈现。
- Grid、fixed/sticky、transform、transition、animation、gradient、background image 和复杂 stacking context 使用 Chromium Web Platform Tests 子集或真实页面冒烟验证。
- 双标签切换不重新加载，加载图标、标题、URL 和 favicon 与 CEF callback 一致。
- Cookie/localStorage 重启后恢复，普通和无痕 profile 隔离。
- 发布 `.app` 在干净机器启动，Renderer/GPU Helper 存在且签名验证通过。

## 5. 非目标

- 不继续扩展自研 CSS parser/layout/paint 以追求网页兼容。
- 不通过 Electron 重写 Browser Chrome，不通过远程调试协议控制外部 Chrome 窗口。
- 不把 Browser 权限、SQLite 或 `archetype://` 特权暴露给 Chromium 网站进程。
- 第一阶段不承诺 Chrome 扩展商店、Google 账号同步、DRM 或 Chrome 品牌服务。

## 6. 完成条件

只有普通网页产品路径完全切到 CEF、导航/标签/profile/打包闭环通过且 legacy fallback 默认关闭后，Feature 才能标记“已完成”。
