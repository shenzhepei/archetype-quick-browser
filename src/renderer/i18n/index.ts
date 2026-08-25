import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'

export const resources = {
  en: {
    translation: {
      appName: 'Archetype',
      newTab: 'New tab',
      closeTab: 'Close tab',
      back: 'Back',
      forward: 'Forward',
      reload: 'Reload',
      stop: 'Stop loading',
      addressPlaceholder: 'Search or enter address',
      bookmark: 'Bookmark this page',
      removeBookmark: 'Remove bookmark',
      profile: 'Profile and settings',
      menu: 'Main menu',
      siteInfo: 'Site information',
      secureConnection: 'Connection is secure',
      verifyingConnection: 'Checking secure connection',
      insecureConnection: 'Connection is not secure',
      localPage: 'Local page',
      internalPage: 'Archetype internal page',
      noSiteInfo: 'No site information',
      history: 'History',
      settings: 'Settings',
      appearance: 'Appearance',
      about: 'About Archetype',
      theme: 'Theme',
      system: 'System',
      light: 'Light',
      dark: 'Dark',
      language: 'Language',
      english: 'English',
      chinese: '简体中文',
      clearHistory: 'Clear history',
      emptyHistory: 'No browsing history yet',
      version: 'Version {{version}}',
      chromium: 'Web content is rendered by Electron Chromium.',
      checkingUpdates: 'Checking for updates...',
      upToDate: 'Archetype is up to date.',
      updateAvailable: 'Version {{version}} is available.',
      noRelease: 'No GitHub Release has been published yet.',
      updateUnavailable: 'Unable to check GitHub Releases.',
      checkAgain: 'Check again',
      viewRelease: 'View release',
      openBookmark: 'Open {{title}}'
    }
  },
  'zh-CN': {
    translation: {
      appName: 'Archetype',
      newTab: '新标签页',
      closeTab: '关闭标签页',
      back: '后退',
      forward: '前进',
      reload: '重新加载',
      stop: '停止加载',
      addressPlaceholder: '搜索或输入网址',
      bookmark: '收藏此页面',
      removeBookmark: '取消收藏',
      profile: '用户与设置',
      menu: '主菜单',
      siteInfo: '站点信息',
      secureConnection: '连接安全',
      verifyingConnection: '正在验证安全连接',
      insecureConnection: '连接不安全',
      localPage: '本地页面',
      internalPage: 'Archetype 内部页面',
      noSiteInfo: '没有站点信息',
      history: '历史记录',
      settings: '设置',
      appearance: '外观',
      about: '关于 Archetype',
      theme: '主题',
      system: '跟随系统',
      light: '浅色',
      dark: '深色',
      language: '语言',
      english: 'English',
      chinese: '简体中文',
      clearHistory: '清除历史记录',
      emptyHistory: '暂无浏览记录',
      version: '版本 {{version}}',
      chromium: '网页内容由 Electron Chromium 渲染。',
      checkingUpdates: '正在检查更新...',
      upToDate: 'Archetype 已是最新版本。',
      updateAvailable: '发现新版本 {{version}}。',
      noRelease: 'GitHub 尚未发布 Release。',
      updateUnavailable: '暂时无法检查 GitHub Release。',
      checkAgain: '重新检查',
      viewRelease: '查看 Release',
      openBookmark: '打开 {{title}}'
    }
  }
} as const

const storedLanguage = localStorage.getItem('archetype-language')
const language = storedLanguage === 'zh-CN' ? 'zh-CN' : 'en'

void i18n.use(initReactI18next).init({
  resources,
  lng: language,
  fallbackLng: 'en',
  interpolation: { escapeValue: false }
})

document.documentElement.lang = language

export async function setLanguage(language: 'en' | 'zh-CN'): Promise<void> {
  localStorage.setItem('archetype-language', language)
  document.documentElement.lang = language
  if (i18n.language !== language) await i18n.changeLanguage(language)
}

export default i18n
