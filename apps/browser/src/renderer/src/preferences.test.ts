import { describe, expect, test } from 'vitest'
import { persistPreferences, readPreferences } from './preferences.js'

function memoryStorage(initial: Record<string, string> = {}): Storage {
  const values = new Map(Object.entries(initial))
  return {
    get length() { return values.size },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => { values.delete(key) },
    setItem: (key, value) => { values.set(key, value) }
  }
}

describe('browser preferences', () => {
  test('uses English when stored values are missing or invalid', () => {
    expect(readPreferences(memoryStorage())).toEqual({})
    expect(readPreferences(memoryStorage({ 'archetype.language': 'fr', 'archetype.theme': 'purple' }))).toEqual({})
  })

  test('persists and restores Simplified Chinese and theme selection', () => {
    const storage = memoryStorage()
    persistPreferences(storage, { language: 'zh-CN', theme: 'dark' })
    expect(readPreferences(storage)).toEqual({ language: 'zh-CN', theme: 'dark' })
  })
})
