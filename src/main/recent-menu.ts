import { isRecordableHistoryEntry } from '../shared/browser'
import type { Bookmark, HistoryEntry } from '../shared/browser'

export function compactMenuTitle(title: string, url: string, maximum = 48): string {
  const value = title.replace(/\s+/g, ' ').trim() || url
  return value.length > maximum ? `${value.slice(0, Math.max(1, maximum - 1))}…` : value
}

export function recentHistory(entries: HistoryEntry[], limit = 8): HistoryEntry[] {
  return entries
    .filter((entry) => isRecordableHistoryEntry(entry.title, entry.url))
    .slice(0, Math.max(0, limit))
}

export function recentBookmarks(bookmarks: Bookmark[], limit = 8): Bookmark[] {
  return [...bookmarks]
    .sort((left, right) => right.createdAt - left.createdAt)
    .slice(0, Math.max(0, limit))
}
