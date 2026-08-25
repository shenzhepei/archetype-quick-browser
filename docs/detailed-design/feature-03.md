# Feature 03 标签页加载状态提示详细设计

| 字段 | 内容 |
|------|------|
| Feature ID | `feature-03` |
| 状态 | 已完成 |
| 对应 PRD | [feature-03.md](../prd/feature-03.md) |
| 主要模块 | `arch-browser::ui` |

## 1. 现有状态

`QuickBrowser::loading_pages` 以 Page ID 记录正在执行的导航。`start_render_request` 在异步任务启动前插入 ID，`finish_render` 在处理成功或失败结果前移除 ID，`stop_loading` 在用户停止时移除 ID；这些路径均调用 `cx.notify()` 触发重绘。工具栏已用同一集合决定显示刷新还是停止按钮。

因此标签图标只需按 Page ID 查询现有集合，无需新增第二套加载状态或修改网络层。原实现曾用 `selected_page_is_loading` 只派生当前标签状态，并在标签栏下渲染水平进度条；修订后删除该派生函数和全局进度元素。

## 2. 标签图标模式

提取纯函数根据加载状态和 favicon 是否存在选择模式：

```text
tab_icon_mode(is_loading, has_favicon)
  false, false -> Default
  false, true  -> Favicon
  true,  *     -> Loading
```

每个标签按自己的 Page ID 读取 `loading_pages`，因此后台标签也能独立显示 loading。空白页和内部页不发起网络导航，不会进入 `loading_pages`。

## 3. UI 结构

标签标题前增加固定图标槽：

```text
tab
  site icon slot (conditional)
    loading ring (loading only)
    favicon or default Globe 12 px (loading)
    favicon or default Globe 16 px (completed)
  title
  close button
```

- 图标槽为 16 x 16 px，相对定位且禁止伸缩。
- loading 环使用组件库既有 Loader 图标和循环旋转动画，尺寸为 16 px。
- 网站图标使用绝对定位叠放在 loading 环中心；加载时为 12 x 12 px，完成后为 16 x 16 px。
- 网站没有 favicon、favicon 尚未取得或获取失败时，使用组件库 `IconName::Globe` 作为默认图标。
- loading 环与 favicon 是两个图层，仅外层 Loader 旋转。
- 所有标签均保留 16 x 16 px 图标槽，favicon 异步到达和加载状态切换不会推动标题。
- 根布局删除标签栏下方原有 2 px 进度条，工具栏重新直接衔接标题栏。

## 4. 动效与进度语义

当前加载管线只报告请求开始和最终结果，没有可靠的总字节数、重定向阶段、图片总数或渲染完成比例。实现采用不定进度旋转动画，而不是将时间映射成百分比。动画仅代表对应标签仍在加载，不承诺完成时间。

## 5. 生命周期与故障行为

| 事件 | `loading_pages` | 对应标签图标 |
|------|-----------------|----------------|
| 标签开始导航 | 插入 Page ID | 显示 loading 环；真实或默认网站图标缩到环内 |
| 标签加载成功 | 移除 Page ID | 环消失；真实或默认网站图标恢复 16 px |
| 标签加载失败 | 移除 Page ID | 环消失；真实或默认网站图标恢复，错误视图继续显示 |
| 用户停止 | 移除 Page ID 并取消任务 | 环消失；真实或默认网站图标恢复 |
| 切换标签 | 集合不变 | 所有标签继续显示各自状态 |
| 后台标签完成 | 移除后台 Page ID | 只更新该后台标签图标 |

## 6. 测试

- `false + false` 选择 Default。
- `false + true` 选择 Favicon。
- `true + false/true` 均选择 Loading。
- 编译和严格 Clippy 验证 GPUI 动画与主题 API。
- `arch-browser` 回归测试验证现有导航、标签和空白页行为。

## 7. 已知边界

- loading 环是状态指示器，不是可访问的下载百分比。
- 页面主文档和渲染任务共用一次加载生命周期；未来若支持流式子资源进度，应建立新的聚合模型，而不是修改本动画的时间曲线。
