import { internalPageTitle } from '../src/shared/browser'

it('localizes every settings section title', () => {
  expect(internalPageTitle('archetype://settings/appearance', 'en')).toBe('Settings - Appearance')
  expect(internalPageTitle('archetype://settings/languages', 'en')).toBe('Settings - Language')
  expect(internalPageTitle('archetype://settings/about', 'zh-CN')).toBe('设置 - 关于 Archetype')
})

it('localizes the history title', () => {
  expect(internalPageTitle('archetype://history', 'zh-CN')).toBe('历史记录')
})
