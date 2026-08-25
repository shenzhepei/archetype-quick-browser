# Feature 04 浏览器内部页与历史记录详细设计

| 字段 | 内容 |
|------|------|
| Feature ID | `feature-04` |
| 状态 | 已完成 |
| 对应 PRD | [feature-04.md](../prd/feature-04.md) |
| 主要模块 | `arch-store`、`arch-browser::BrowserCore`、`arch-browser::ui`、`i18n` |

## 1. 现有行为与问题

`pages` 只保存每个全局标签的当前 URL、标题和最近访问时间。后退/前进栈主要由内存 Session 所有，休眠快照只服务标签恢复，因此不能作为用户可管理的长期访问历史。工具栏头像只承载外观设置，右侧没有浏览器主菜单，也没有稳定的内部页面路由。

## 2. SQLite schema v5

新增独立表：

```sql
CREATE TABLE history_entries (
  id TEXT PRIMARY KEY,
  url TEXT NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  visited_at INTEGER NOT NULL
);
CREATE INDEX history_entries_visited_at
  ON history_entries(visited_at DESC, id DESC);
```

- ID 使用 UUID v7，在同毫秒记录之间保持稳定次序。
- 不保存 `page_id` 外键，关闭标签不会删除历史。
- `Store::commit_page_navigation` 在一个 `BEGIN IMMEDIATE` 事务中更新 `pages` 并插入历史；页面不存在时不插入孤立记录。
- `Store::history_entries(1000)` 按 `visited_at DESC, id DESC` 有界读取。
- 删除单条按 ID 执行，清空使用单条 `DELETE`。

## 3. BrowserCore 边界

`finish_navigation` 先沿用 Navigation ID 检查并提交 Session 最终 URL，只有最新成功结果才能调用事务接口，因此陈旧结果不会进入历史。BrowserCore 公开有界读取、删除和清空包装，不把 rusqlite 类型暴露给 UI。

历史记录保存最终重定向 URL 和渲染后的标题。内部页面不经过 `finish_navigation`，自然不会写入；UI 仍通过 `is_internal_page` 明确阻止其 reload/恢复加载。

## 4. 内部页面状态

内部路由白名单：

| URL | 视图 |
|-----|------|
| `archetype://history` | 浏览历史 |
| `archetype://settings/appearance` | 外观设置 |
| `archetype://settings/about` | 关于 Archetype |

历史页打开流程：

```text
open history menu
  existing history tab -> select + refresh entries
  no history tab       -> create persisted Page + select + refresh entries

select/restore history tab
  -> render native history content
  -> never call Loader or Runtime
```

同类内部页在全局标签中保持唯一并可跨重启恢复。地址栏显示内部 URL；用户在内部页地址栏输入普通 URL 时创建新的普通标签，避免把内部 URL 混入网络 Session 历史。

`archetype://` 不映射任意磁盘路径。路由匹配发生在 Browser UI，白名单之外的内部 URL显示受控空状态。当前使用 GPUI 原生视图实现交互，因为静态 HTML 引擎没有可安全调用 Browser 设置 API 的事件桥；未来 bundled HTML WebUI 只能通过显式消息协议替换视图，不能获得网络或 profile 数据库直接访问权。

## 5. UI 结构

- 工具栏顺序为：导航按钮、地址栏、头像快捷入口、三点主菜单。
- 三点按钮使用现有 `IconName::EllipsisVertical` 和 DropdownMenu。
- 菜单包含“历史记录”和“设置”；头像直接打开外观设置。
- 历史页为安静、紧凑的工作界面：标题和清空命令、筛选输入、可滚动记录列表。
- 每行固定显示标题、URL、访问时间和删除图标；点击主体新建普通标签导航，删除按钮停止事件传播。
- 页面最多持有最近 1000 条记录，筛选在内存中对标题和 URL 做不区分大小写匹配。
- 设置视图采用左侧固定导航和右侧内容区；外观使用 Radio，关于页从 `env!("CARGO_PKG_VERSION")` 读取版本。

## 6. 状态与数据流

`QuickBrowser` 持有 `history_entries` 与 `history_filter`。打开内部页时从 BrowserCore 刷新；输入变化只触发重绘；删除和清空先提交 Store，成功后更新内存列表。数据库错误复用现有应用错误视图。历史记录点击和内部页地址栏导航均创建普通标签，避免把 `archetype:` URL 混入可由网络 Loader 回退的 Session 历史。

内部设置分区切换调用无历史写入的页面元数据更新接口，只更新目标设置标签 URL。外观单选继续调用既有 `set_appearance`，因此 SQLite `app_state.appearance` 与 GPUI ThemeMode 语义不变。

## 7. 失败与安全行为

- 创建历史标签失败时显示应用错误，不创建半成品 UI 状态。
- 读取历史失败时保留现有列表并显示错误。
- 删除失败时不从内存列表移除，避免 UI 与数据库不一致。
- 历史 URL 点击后继续使用 `parse_address`/`navigate_to` 的正常安全路径。
- Renderer 不接收历史数据，网站内容无法访问内部页模型。

## 8. 测试矩阵

| 层 | 测试 |
|----|------|
| Store | schema v5、原子导航记录、倒序上限、删除和清空 |
| BrowserCore | 成功导航产生记录；陈旧导航不产生记录 |
| UI | `archetype://history` 识别、内部页跳过加载、标题/URL 筛选 |
| Settings | 路由识别、设置标签唯一性、外观偏好和版本文案 |
| Regression | `arch-store`、`arch-browser`、workspace、严格 Clippy、支持矩阵 |

## 9. 已知边界

- 首版不分页，界面只加载最近 1000 条。
- 访问时间按本机当前时区显示；切换时区后旧记录按新时区重新格式化。
- 清空历史不清理 Cookie、缓存、书签、标签或休眠快照；统一“清除浏览数据”属于后续 Feature。
