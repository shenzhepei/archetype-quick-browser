import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'

void i18n.use(initReactI18next).init({
  lng: 'en',
  fallbackLng: 'en',
  interpolation: { escapeValue: false },
  resources: {
    en: { translation: {
      newTab: 'New tab', back: 'Back', forward: 'Forward', reload: 'Reload', address: 'Search or enter address',
      runtime: 'Runtime', runtimeReady: 'Runtime connected', runtimeMissing: 'This site has not configured Archetype Runtime.',
      project: 'Project', operations: 'Operations', identity: 'Identity', signedOut: 'Not signed in', appearance: 'Appearance', language: 'Language',
      tabs: 'Tabs', closeTab: 'Close {{title}}', noProject: 'No project discovered', openConfiguredSite: 'Open a configured HTTPS application.',
      noOperations: 'No callable operations', originSession: 'Origin-bound OIDC session', authOnDemand: 'Authentication is requested by protected functions',
      trustBoundary: 'Trust boundary', trustBoundaryDescription: 'Websites can invoke declared capabilities. Database addresses, credentials, OIDC tokens, device private keys and arbitrary SQL remain outside page JavaScript.',
      documentTitle: 'Archetype Runtime Browser', documentDescription: 'A browser-native runtime for trusted self-hosted functions, data, and durable jobs.'
    } },
    'zh-CN': { translation: {
      newTab: '新标签页', back: '后退', forward: '前进', reload: '刷新', address: '搜索或输入地址',
      runtime: '运行时', runtimeReady: '运行时已连接', runtimeMissing: '此网站尚未配置 Archetype Runtime。',
      project: '项目', operations: '函数', identity: '身份', signedOut: '未登录', appearance: '外观', language: '语言',
      tabs: '标签页', closeTab: '关闭 {{title}}', noProject: '未发现项目', openConfiguredSite: '请打开已配置的 HTTPS 应用。',
      noOperations: '没有可调用函数', originSession: '绑定 Origin 的 OIDC 会话', authOnDemand: '受保护函数会按需请求身份验证',
      trustBoundary: '信任边界', trustBoundaryDescription: '网站可以调用已声明的能力。数据库地址、凭证、OIDC Token、设备私钥和任意 SQL 始终位于页面 JavaScript 之外。',
      documentTitle: 'Archetype Runtime 浏览器', documentDescription: '面向可信自托管云函数、数据与可靠任务的浏览器原生运行时。'
    } }
  }
})

export default i18n
