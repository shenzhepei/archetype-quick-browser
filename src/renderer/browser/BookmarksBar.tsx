import { ChevronsRight } from 'lucide-react'
import { useLayoutEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { ArchetypeBridge, Bookmark } from '../../shared/browser'
import { BookmarkFavicon } from './BookmarkFavicon'

export function visibleBookmarkCount(widths: number[], availableWidth: number, moreWidth = 32, gap = 3): number {
  const totalWidth = widths.reduce((total, width) => total + width, 0) + Math.max(0, widths.length - 1) * gap
  if (totalWidth <= availableWidth) return widths.length
  const capacity = Math.max(0, availableWidth - moreWidth - gap)
  let used = 0
  for (let index = 0; index < widths.length; index += 1) {
    const next = widths[index] + (index > 0 ? gap : 0)
    if (used + next > capacity) return index
    used += next
  }
  return widths.length
}

export function BookmarksBar({ bookmarks, bridge }: { bookmarks: Bookmark[]; bridge: ArchetypeBridge }): React.JSX.Element | null {
  const { t } = useTranslation()
  const barRef = useRef<HTMLElement>(null)
  const moreRef = useRef<HTMLButtonElement>(null)
  const buttonRefs = useRef(new Map<string, HTMLButtonElement>())
  const [visibleCount, setVisibleCount] = useState(bookmarks.length)
  const measurementKey = bookmarks.map((bookmark) => `${bookmark.id}:${bookmark.title}`).join('|')

  useLayoutEffect(() => {
    const measure = (): void => {
      const bar = barRef.current
      if (!bar) return
      const widths = bookmarks.map((bookmark) => buttonRefs.current.get(bookmark.id)?.offsetWidth ?? 0)
      const available = Math.max(0, bar.clientWidth - 20)
      setVisibleCount(visibleBookmarkCount(widths, available, moreRef.current?.offsetWidth || 32))
    }
    measure()
    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(measure)
    if (barRef.current) observer.observe(barRef.current)
    return () => observer.disconnect()
  }, [bookmarks, measurementKey])

  if (bookmarks.length === 0) return null
  const overflow = bookmarks.slice(visibleCount)
  return (
    <nav
      className="bookmarks-bar"
      aria-label={t('bookmarks')}
      ref={barRef}
      onContextMenu={(event) => {
        event.preventDefault()
        void bridge.showBookmarksBarMenu({ x: event.clientX, y: event.clientY })
      }}
    >
      {bookmarks.map((bookmark, index) => (
        <button
          className={`bookmark-item ${index >= visibleCount ? 'is-overflow' : ''}`}
          key={bookmark.id}
          ref={(element) => { if (element) buttonRefs.current.set(bookmark.id, element); else buttonRefs.current.delete(bookmark.id) }}
          title={bookmark.url}
          aria-label={t('openBookmark', { title: bookmark.title })}
          onClick={() => void bridge.navigate(bookmark.url)}
        >
          <BookmarkFavicon favicon={bookmark.favicon} size={13} /><span>{bookmark.title}</span>
        </button>
      ))}
      <button
        className={`bookmarks-more ${overflow.length > 0 ? '' : 'is-hidden'}`}
        ref={moreRef}
        aria-label={t('moreBookmarks')}
        title={t('moreBookmarks')}
        onClick={(event) => {
          const rect = event.currentTarget.getBoundingClientRect()
          void bridge.showBookmarksOverflowMenu({ x: rect.left, y: rect.bottom, bookmarkIds: overflow.map((bookmark) => bookmark.id) })
        }}
      >
        <ChevronsRight size={17} />
      </button>
    </nav>
  )
}
