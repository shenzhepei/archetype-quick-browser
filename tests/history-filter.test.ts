import { isRecordableHistoryEntry } from '../src/shared/browser'

it('only records titled HTTP and HTTPS pages', () => {
  expect(isRecordableHistoryEntry('Example', 'https://example.com')).toBe(true)
  expect(isRecordableHistoryEntry('Example', 'http://example.com')).toBe(true)
  expect(isRecordableHistoryEntry('New tab', 'https://example.com')).toBe(false)
  expect(isRecordableHistoryEntry('新标签页', 'https://example.com')).toBe(false)
  expect(isRecordableHistoryEntry('  ', 'https://example.com')).toBe(false)
  expect(isRecordableHistoryEntry('Blank', 'about:blank')).toBe(false)
  expect(isRecordableHistoryEntry('Settings', 'archetype://settings/appearance')).toBe(false)
})
