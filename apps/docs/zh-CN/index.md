---
layout: home
title: Archetype Runtime
titleTemplate: 浏览器原生应用运行时
hero:
  name: Archetype Runtime
  text: 前端体验，可信执行
  tagline: 通过绑定Origin的浏览器API调用自托管Node.js云函数、PostgreSQL/MySQL事务和可靠队列。
  actions:
    - theme: brand
      text: 浏览器指南
      link: /zh-CN/guide/browser
    - theme: alt
      text: 自托管
      link: /zh-CN/guide/self-hosting
    - theme: alt
      text: 下载浏览器
      link: https://github.com/shenzhepei/archetype-runtime-browser/releases
features:
  - title: 浏览器原生
    details: 顶层HTTPS应用获得navigator.archetype，但不会获得数据库凭证或身份令牌。
  - title: 自托管
    details: Gateway、函数宿主、Worker、PostgreSQL、MySQL和Caddy运行在你的Docker环境。
  - title: 事务可靠
    details: 原子业务更新与Outbox为带重试、租约和死信的任务提供事件来源。
---

![Archetype Runtime 浏览器](/preview.webp)

> 首版安装包未签名。Windows SmartScreen和macOS Gatekeeper可能提示发布者未知。本项目不提供绕过操作系统安全控制的脚本。
