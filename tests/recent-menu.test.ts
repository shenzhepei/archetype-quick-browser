import { compactMenuTitle, recentBookmarks, recentHistory } from '../src/main/recent-menu'

it('limits recent history without changing visit order', () => {
  const entries = Array.from({ length: 10 }, (_, index) => ({
    id: `${index}`,
    title: `Page ${index}`,
    url: `https://example.com/${index}`,
    visitedAt: 10 - index
  }))
  expect(recentHistory(entries).map((entry) => entry.id)).toEqual(['0', '1', '2', '3', '4', '5', '6', '7'])
})

it('filters new-tab placeholders and non-web pages from recent history', () => {
  const entries = [
    { id: 'new', title: 'New tab', url: 'https://example.com', visitedAt: 3 },
    { id: 'blank', title: 'Blank', url: 'about:blank', visitedAt: 2 },
    { id: 'valid', title: 'Example', url: 'https://example.com', visitedAt: 1 }
  ]
  expect(recentHistory(entries).map((entry) => entry.id)).toEqual(['valid'])
})

it('sorts bookmarks by creation time and compacts menu titles', () => {
  const bookmarks = [
    { id: 'old', title: 'Old', url: 'https://old.example', createdAt: 1 },
    { id: 'new', title: 'New', url: 'https://new.example', createdAt: 2 }
  ]
  expect(recentBookmarks(bookmarks).map((bookmark) => bookmark.id)).toEqual(['new', 'old'])
  expect(compactMenuTitle('  A   title  ', 'https://example.com')).toBe('A title')
  expect(compactMenuTitle('', 'https://example.com', 10)).toBe('https://e…')
})
