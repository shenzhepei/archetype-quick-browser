import { Globe2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { ArchetypeBridge, Bookmark } from '../../shared/browser'

export function BookmarksBar({ bookmarks, bridge }: { bookmarks: Bookmark[]; bridge: ArchetypeBridge }): React.JSX.Element | null {
  const { t } = useTranslation()
  if (bookmarks.length === 0) return null
  return (
    <nav className="bookmarks-bar" aria-label="Bookmarks">
      {bookmarks.map((bookmark) => (
        <button key={bookmark.id} title={bookmark.url} aria-label={t('openBookmark', { title: bookmark.title })} onClick={() => void bridge.navigate(bookmark.url)}>
          <Globe2 size={13} /><span>{bookmark.title}</span>
        </button>
      ))}
    </nav>
  )
}
