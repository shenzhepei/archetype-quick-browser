# 浏览器指南

Archetype Runtime 是专注于应用运行的Electron Chromium浏览器。它保留熟悉的导航与标签页，并为配置过的HTTPS应用增加可信能力边界。

## 安装与使用

从GitHub Releases下载对应平台的安装包。首版未签名且未公证，因此Windows SmartScreen或macOS Gatekeeper可能提示发布者未知。打开前请核对Release校验和；Archetype不提供关闭操作系统安全检查的脚本。

像普通浏览器一样打开HTTPS应用。工具栏显示站点权限和Runtime状态；`archetype://runtime` 显示当前发现的项目、登录用户、Gateway健康状态，以及授予当前Origin的操作。

## 站点发现

应用在同源精确路径 `/.well-known/archetype-runtime.json` 提供：

```json
{
  "version": 1,
  "projectId": "shop-production",
  "gatewayUrl": "https://runtime.example.com"
}
```

远程Gateway必须使用HTTPS；本地开发允许 `http://localhost`。

## 可用范围

`navigator.archetype` 只存在于顶层HTTPS或localhost文档，不注入远程HTTP、文件、内部页、iframe或Service Worker。发现过程还会验证真实Frame Origin是否被项目允许。

```ts
const project = await navigator.archetype.discover()
console.log(project.operations)
```

Electron主进程负责发现、身份和签名请求。页面JavaScript不会获得数据库URL、OIDC令牌、能力票据或设备私钥。

## 登录与设备绑定

`signIn()`在浏览器中启动OIDC Authorization Code + PKCE，由Gateway接收回调并交换Token。OIDC访问令牌和刷新令牌始终保留在Gateway，网站只得到安全的会话摘要。

Electron为每个项目和Origin生成独立的Ed25519设备密钥，并使用Electron `safeStorage` 加密私钥。Gateway签发绑定该公钥的60秒能力票据。这能防止普通Token复制与重放，但不等同于硬件远程证明。

```ts
const session = await navigator.archetype.signIn()
console.log(session.displayName)
await navigator.archetype.signOut()
```
