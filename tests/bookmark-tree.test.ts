import { buildBookmarkTree } from '../src/shared/bookmark-tree'

it('builds nested bookmark folders and places bookmarks at their parent', () => {
  const folders = [
    { id: 'parent', name: 'Parent', createdAt: 1 },
    { id: 'child', name: 'Child', parentId: 'parent', createdAt: 2 }
  ]
  const bookmarks = [
    { id: 'root-bookmark', title: 'Root', url: 'https://root.example', createdAt: 1 },
    { id: 'child-bookmark', title: 'Child page', url: 'https://child.example', parentId: 'child', createdAt: 2 }
  ]

  const tree = buildBookmarkTree(bookmarks, folders)
  expect(tree.bookmarks.map((bookmark) => bookmark.id)).toEqual(['root-bookmark'])
  expect(tree.folders[0].folders[0].bookmarks.map((bookmark) => bookmark.id)).toEqual(['child-bookmark'])
})

it('falls orphaned and cyclic folders and bookmarks back to the root', () => {
  const folders = [
    { id: 'orphan', name: 'Orphan', parentId: 'missing', createdAt: 1 },
    { id: 'cycle-a', name: 'Cycle A', parentId: 'cycle-b', createdAt: 2 },
    { id: 'cycle-b', name: 'Cycle B', parentId: 'cycle-a', createdAt: 3 }
  ]
  const bookmarks = [{ id: 'orphan-bookmark', title: 'Orphan page', url: 'https://orphan.example', parentId: 'missing', createdAt: 1 }]

  const tree = buildBookmarkTree(bookmarks, folders)
  expect(tree.folders.map((folder) => folder.id)).toEqual(['orphan', 'cycle-a', 'cycle-b'])
  expect(tree.bookmarks.map((bookmark) => bookmark.id)).toEqual(['orphan-bookmark'])
})
