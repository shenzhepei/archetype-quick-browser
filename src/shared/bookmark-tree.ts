import type { Bookmark, BookmarkFolder } from './browser'

export interface BookmarkFolderNode extends BookmarkFolder {
  folders: BookmarkFolderNode[]
  bookmarks: Bookmark[]
}

export interface BookmarkTree {
  folders: BookmarkFolderNode[]
  bookmarks: Bookmark[]
}

export function buildBookmarkTree(bookmarks: Bookmark[], folders: BookmarkFolder[]): BookmarkTree {
  const folderMap = new Map(folders.map((folder) => [folder.id, folder]))
  const safeParent = new Map<string, string | undefined>()
  for (const folder of folders) {
    const visited = new Set([folder.id])
    let parentId = folder.parentId
    let valid = true
    while (parentId) {
      if (visited.has(parentId) || !folderMap.has(parentId)) {
        valid = false
        break
      }
      visited.add(parentId)
      parentId = folderMap.get(parentId)?.parentId
    }
    safeParent.set(folder.id, valid ? folder.parentId : undefined)
  }

  const nodes = new Map<string, BookmarkFolderNode>(
    folders.map((folder) => [folder.id, { ...folder, folders: [], bookmarks: [] }])
  )
  const roots: BookmarkFolderNode[] = []
  for (const folder of folders) {
    const node = nodes.get(folder.id)!
    const parent = safeParent.get(folder.id)
    if (parent) nodes.get(parent)!.folders.push(node)
    else roots.push(node)
  }
  const rootBookmarks: Bookmark[] = []
  for (const bookmark of bookmarks) {
    const parent = bookmark.parentId ? nodes.get(bookmark.parentId) : undefined
    if (parent) parent.bookmarks.push(bookmark)
    else rootBookmarks.push(bookmark)
  }
  const sortNodes = (items: BookmarkFolderNode[]): void => {
    items.sort((left, right) => left.createdAt - right.createdAt)
    for (const item of items) sortNodes(item.folders)
  }
  sortNodes(roots)
  return { folders: roots, bookmarks: rootBookmarks }
}
