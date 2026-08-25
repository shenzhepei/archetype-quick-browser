# Feature 05 地址栏站点安全信息 PRD

| 字段 | 内容 |
| --- | --- |
| Feature | `feature-05` |
| 状态 | 已实现 |
| 对应详设 | [feature-05.md](../detailed-design/feature-05.md) |
| 所属规范 | [01 Archetype Chromium 浏览器](01-Archetype-Chromium浏览器-PRD.md) |

## 1. 背景

地址栏当前只显示 URL，用户无法区分 HTTPS、明文 HTTP、内部页和本地页，也无法查看 Chromium 实际验证的证书或当前 origin 请求过的权限。

## 2. 需求与验收

| ID | 需求 | 验收标准 |
| --- | --- | --- |
| F05-01 | 地址栏状态 | 地址栏左侧显示站点信息按钮，区分安全 HTTPS、不安全 HTTP、内部/本地页面和无站点状态 |
| F05-02 | 真实验证 | HTTPS 安全结论来自 Chromium 证书验证结果；不得仅根据 URL scheme 宣称证书有效 |
| F05-03 | 证书摘要 | 站点信息菜单显示证书是否有效；可查看 subject、issuer、有效期、指纹和 known-root 状态 |
| F05-04 | 默认验证 | 证书采集不得放行无效证书或改变 Chromium 默认校验行为 |
| F05-05 | 权限状态 | 显示当前 origin 实际请求过的权限及 granted/blocked 状态；没有记录时明确显示无已授权权限 |
| F05-06 | 原生层级 | 站点信息和证书详情位于网页 `WebContentsView` 之上，不被网页内容遮挡 |
| F05-07 | 国际化 | 安全状态、证书字段、权限名称和状态提供英文与简体中文 |
| F05-08 | 安全边界 | Renderer 只能请求当前活动标签的站点信息菜单，不能指定 origin、证书或任意菜单模板 |

## 3. 权限策略

本 Feature 保持现有默认拒绝策略，不因增加状态展示自动授予摄像头、麦克风、定位或通知权限。实际 permission request 会按 origin 记录为 blocked；已授权列表为空时如实显示。授权提示与持久化决策属于后续独立 Feature。

## 4. 非目标

- 不绕过证书错误页，不允许用户在本 Feature 中信任无效证书。
- 不实现完整 Chrome Page Info 弹窗、Cookie 明细、站点数据清理或权限授权弹窗。
