import { internalPageTitle } from '../src/shared/browser'

it('localizes every settings section title', () => {
  expect(internalPageTitle('archetype://settings/appearance', 'en')).toBe('Settings - Appearance')
  expect(internalPageTitle('archetype://settings/languages', 'en')).toBe('Settings - Language')
  expect(internalPageTitle('archetype://settings/about', 'zh-CN')).toBe('设置 - 关于 Archetype')
  expect(internalPageTitle('archetype://extensions', 'en')).toBe('Extensions')
  expect(internalPageTitle('archetype://bookmarks', 'en')).toBe('Bookmarks')
})

it('localizes the history title', () => {
  expect(internalPageTitle('archetype://history', 'zh-CN')).toBe('历史记录')
  expect(internalPageTitle('archetype://extensions', 'zh-CN')).toBe('扩展程序')
  expect(internalPageTitle('archetype://bookmarks', 'zh-CN')).toBe('书签')
})
