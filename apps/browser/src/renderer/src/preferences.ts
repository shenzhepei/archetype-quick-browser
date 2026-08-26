import type { Language, Theme } from '../../shared.js'

const languageKey = 'archetype.language'
const themeKey = 'archetype.theme'

export function readPreferences(storage: Pick<Storage, 'getItem'>): { language?: Language; theme?: Theme } {
  const language = storage.getItem(languageKey)
  const theme = storage.getItem(themeKey)
  return {
    ...(language === 'en' || language === 'zh-CN' ? { language } : {}),
    ...(theme === 'system' || theme === 'light' || theme === 'dark' ? { theme } : {})
  }
}

export function persistPreferences(storage: Pick<Storage, 'setItem'>, value: { language?: Language; theme?: Theme }): void {
  if (value.language) storage.setItem(languageKey, value.language)
  if (value.theme) storage.setItem(themeKey, value.theme)
}
