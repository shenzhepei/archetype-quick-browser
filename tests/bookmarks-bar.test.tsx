import { fireEvent, render, screen } from '@testing-library/react'
import type { ArchetypeBridge, Bookmark } from '../src/shared/browser'
import { BookmarksBar, visibleBookmarkCount } from '../src/renderer/browser/BookmarksBar'
import '../src/renderer/i18n'

const favicon = 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw=='

it('shows persisted favicons and falls back for bookmarks without one', () => {
  const bookmarks: Bookmark[] = [
    { id: 'with-icon', title: 'With icon', url: 'https://example.com', favicon, createdAt: 2 },
    { id: 'without-icon', title: 'Without icon', url: 'https://example.org', createdAt: 1 }
  ]
  const bridge = { navigate: vi.fn(), showBookmarksBarMenu: vi.fn(), showBookmarksOverflowMenu: vi.fn() } as unknown as ArchetypeBridge
  const { container } = render(<BookmarksBar bookmarks={bookmarks} bridge={bridge} />)

  expect(container.querySelector(`img[src="${favicon}"]`)).toBeInTheDocument()
  expect(container.querySelectorAll('.bookmark-favicon svg')).toHaveLength(1)
  fireEvent.click(screen.getByRole('button', { name: 'Open With icon' }))
  expect(bridge.navigate).toHaveBeenCalledWith('https://example.com')
  fireEvent.contextMenu(screen.getByRole('navigation', { name: 'Bookmarks' }), { clientX: 80, clientY: 96 })
  expect(bridge.showBookmarksBarMenu).toHaveBeenCalledWith({ x: 80, y: 96 })
  fireEvent.click(screen.getByRole('button', { name: 'More bookmarks' }))
  expect(bridge.showBookmarksOverflowMenu).toHaveBeenCalledWith({ x: 0, y: 0, bookmarkIds: ['without-icon'] })
})

it('reserves the more button and only fits complete bookmark items', () => {
  expect(visibleBookmarkCount([80, 90, 70], 300)).toBe(3)
  expect(visibleBookmarkCount([80, 90, 70], 220)).toBe(2)
  expect(visibleBookmarkCount([80], 40)).toBe(0)
})
