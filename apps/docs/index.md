---
layout: home
title: Archetype Runtime
titleTemplate: Browser-native application runtime
hero:
  name: Archetype Runtime
  text: Frontend experience. Trusted execution.
  tagline: Invoke self-hosted Node.js functions, PostgreSQL/MySQL transactions, and durable jobs through an origin-bound browser API.
  actions:
    - theme: brand
      text: Browser guide
      link: /guide/browser
    - theme: alt
      text: Self-hosting
      link: /guide/self-hosting
    - theme: alt
      text: Download browser
      link: https://github.com/shenzhepei/archetype-runtime-browser/releases
features:
  - title: Browser-native
    details: Top-level HTTPS applications receive navigator.archetype without receiving database credentials or identity tokens.
  - title: Self-hosted
    details: Gateway, function host, worker, PostgreSQL, MySQL and Caddy run in your Docker environment.
  - title: Transactional
    details: Atomic business updates and an outbox feed leased jobs with retries and dead-letter handling.
---

![Archetype Runtime browser](/preview.webp)

> Release installers are initially unsigned. Windows SmartScreen and macOS Gatekeeper may identify them as an unknown publisher. The project does not provide scripts that bypass operating-system security controls.
