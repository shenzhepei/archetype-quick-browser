import i18n, { resources, setLanguage } from '../src/renderer/i18n'

describe('runtime localization', () => {
  it('keeps locale catalogs structurally equivalent', () => {
    expect(Object.keys(resources.en.translation)).toEqual(Object.keys(resources['zh-CN'].translation))
  })

  it('switches and persists Simplified Chinese', async () => {
    await setLanguage('zh-CN')
    expect(i18n.language).toBe('zh-CN')
    expect(document.documentElement.lang).toBe('zh-CN')
    expect(localStorage.getItem('archetype-language')).toBe('zh-CN')
  })
})
