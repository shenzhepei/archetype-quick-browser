# Feature 02 标签性能与站点兼容性修复详细设计

| 字段 | 内容 |
|------|------|
| Feature ID | `feature-02` |
| 状态 | 已完成 |
| 对应 PRD | [feature-02.md](../prd/feature-02.md) |
| 主要模块 | `arch-browser::ui`、`BrowserCore`、`runtime_broker`、`arch-net` |

## 1. 标签常驻策略

原实现每次选择不同标签时调用 `hibernate_selected_page`：同步序列化快照、写 SQLite、停止导航、删除 `RenderedPage`，切回后同步重载。这使两标签切换退化为磁盘 I/O 和完整渲染。

新策略：

```text
switch(current, next)
  resident < 8       -> keep current RenderedPage
  resident >= 8      -> try hibernate current
  next is resident   -> notify and paint immediately
  next is hibernated -> restore metadata, then async reload
  next is blank      -> paint empty surface
```

- `MAX_RESIDENT_RENDERED_PAGES = 8` 是有界常驻阈值。
- `should_hibernate_on_switch` 只在标签确实变化且达到阈值时返回 true。
- 自动休眠仍调用现有 `hibernate_page(..., automatic=true)`，因此脏表单不会被丢弃。

## 2. 异步休眠恢复

`BrowserCore::resume_page` 只执行以下操作：

1. 读取并反序列化 `HibernationSnapshot`。
2. 校验 snapshot PageId 与目标 Page 一致。
3. 恢复 Session 历史、游标、视口和滚动元数据。
4. 删除持久化 snapshot。

UI 随后调用既有异步 `reload_page`，从而统一经过 Cookie-aware Browser broker、Runtime 和 favicon 管线。`wake_page` 保留为同步 API，并复用 `resume_page + reload`，避免两套恢复语义。

## 3. HTTP 内容协商

`arch-net::Loader` 的 reqwest client 启用：

- `gzip`
- `brotli`
- `deflate`
- `zstd`

Client 设置桌面兼容 User-Agent，并保留产品标识：

```text
Mozilla/5.0 ... Archetype/<CARGO_PKG_VERSION> Chrome/... Safari/...
```

百度会向未知或产品名式 UA 返回仅包含 JavaScript HTTP 降级跳转的极简页面；桌面兼容 UA 返回完整 HTML。Archetype 不执行该跳转脚本，而是请求可静态解析的桌面文档。响应读取继续使用 `limit + 1`，限制作用于解压后的字节流。

## 4. favicon 格式与回退

候选顺序为：

1. 第一个同源 `link[rel~=icon]`。
2. 同源 `/favicon.ico`。

每个候选都经过 Cookie 策略、1 MiB 上限、请求/最终 URL 同源校验。栅格图片由 `image` 解码后缩放为 PNG；UTF-8 SVG 在大小校验后保留原字节，由 GPUI `ImageFormat::Svg` 渲染。显式候选下载或解码失败不会提前终止默认候选。

## 5. `about:blank`

- `add_page` 创建持久 URL `about:blank`，只更新选择、地址输入和标签滚动位置。
- 不调用 `navigate_to`，因此 Loader 不需要支持 `about:` scheme。
- 选择、关闭邻接标签和启动恢复均通过 `is_blank_page` 跳过 reload。
- 内容区域对选中的空白页返回无子元素的全尺寸 surface；没有标签时仍显示既有空状态。
- 用户在地址栏输入真实 URL 后，既有导航会把该标签更新为最终 URL 和标题。

## 6. 测试矩阵

| 层 | 测试 |
|----|------|
| UI 纯逻辑 | 两标签不休眠、同标签不休眠、达到 8 页阈值才休眠 |
| Blank | `blank_url()` 生成 `about:blank`，`is_blank_page` 正确识别 |
| Network | 本地 gzip 响应自动解码；请求包含 Archetype 版本 UA |
| Favicon | SVG 原样保留；栅格图归一化；跨域 favicon 拒绝 |
| Core | 休眠历史恢复、异步/同步恢复共用 metadata 路径 |
| Regression | 全工作区测试、72 页截图、Runtime subprocess、支持矩阵 |

## 7. 已知边界

- 百度页面中的同源 Logo 和静态文字可以进入显示列表；跨域 CDN 图片仍会诊断为拒绝。
- 大量不支持 CSS 和 JavaScript 会影响视觉还原，但不应再次表现为主文档空白。
- SVG 只用于标签 favicon；页面内容图片仍使用现有确定性栅格格式集合。
