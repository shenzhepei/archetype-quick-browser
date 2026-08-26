import { fireEvent, render, screen } from '@testing-library/react'
import type { ArchetypeBridge, BrowserState } from '../src/shared/browser'
import { InternalPage } from '../src/renderer/browser/InternalPage'
import '../src/renderer/i18n'

const bridge = {
  updateSettings: vi.fn(),
  clearHistory: vi.fn(),
  navigate: vi.fn(),
  openInternal: vi.fn(),
  getAppVersion: vi.fn().mockResolvedValue('1.2.3'),
  checkForUpdates: vi.fn().mockResolvedValue({ currentVersion: '1.2.3', latestVersion: '1.2.4', releaseUrl: 'https://github.com/shenzhepei/archetype-quick-browser/releases/tag/v1.2.4', checkedAt: 0, state: 'update-available' }),
  openLatestRelease: vi.fn(),
  listExtensions: vi.fn().mockResolvedValue([{ id: 'sample', name: 'Sample extension', version: '1.0.0', description: 'Test extension', path: '/tmp/sample-extension' }]),
  installExtension: vi.fn().mockResolvedValue({ ok: true, extensions: [] }),
  removeExtension: vi.fn().mockResolvedValue({ ok: true, extensions: [] }),
  removeBookmark: vi.fn(),
  createBookmarkFolder: vi.fn(),
  removeBookmarkFolder: vi.fn(),
  moveBookmark: vi.fn()
} as unknown as ArchetypeBridge
const state: BrowserState = { tabs: [], activeTabId: '', bookmarks: [], bookmarkFolders: [], history: [], settings: { theme: 'system', language: 'en' }, siteInfo: { url: '', connection: 'none', permissions: [] } }

it('changes the appearance preference', () => {
  render(<InternalPage url="archetype://settings/appearance" state={state} bridge={bridge} />)
  fireEvent.click(screen.getByRole('button', { name: 'Dark' }))
  expect(bridge.updateSettings).toHaveBeenCalledWith({ theme: 'dark' })
})

it('changes the language from settings', () => {
  render(<InternalPage url="archetype://settings/languages" state={state} bridge={bridge} />)
  fireEvent.click(screen.getByRole('button', { name: '简体中文' }))
  expect(bridge.updateSettings).toHaveBeenCalledWith({ language: 'zh-CN' })
})

it('renders an empty history state', () => {
  render(<InternalPage url="archetype://history" state={state} bridge={bridge} />)
  expect(screen.getByText('No browsing history yet')).toBeInTheDocument()
})

it('shows the packaged and latest release versions', async () => {
  render(<InternalPage url="archetype://settings/about" state={state} bridge={bridge} />)
  expect(await screen.findByText('Version 1.2.3')).toBeInTheDocument()
  expect(await screen.findByText('Version 1.2.4 is available.')).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'View release' }))
  expect(bridge.openLatestRelease).toHaveBeenCalled()
})

it('lists and removes loaded extensions', async () => {
  render(<InternalPage url="archetype://extensions" state={state} bridge={bridge} />)
  expect(await screen.findByText('Sample extension')).toBeInTheDocument()
  expect(screen.getByText('Version 1.0.0')).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'Remove Sample extension' }))
  expect(bridge.removeExtension).toHaveBeenCalledWith('sample')
})

it('opens and removes bookmarks from the manager', () => {
  const favicon = 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw=='
  const bookmarkState: BrowserState = {
    ...state,
    bookmarks: [{ id: 'bookmark-one', title: 'Example', url: 'https://example.com', favicon, createdAt: 1 }]
  }
  const { container } = render(<InternalPage url="archetype://bookmarks" state={bookmarkState} bridge={bridge} />)
  expect(container.querySelector(`img[src="${favicon}"]`)).toBeInTheDocument()
  fireEvent.click(screen.getByText('Example').closest('button')!)
  expect(bridge.navigate).toHaveBeenCalledWith('https://example.com')
  fireEvent.click(screen.getByRole('button', { name: 'Remove Example' }))
  expect(bridge.removeBookmark).toHaveBeenCalledWith('bookmark-one')
})

it('creates nested folders, moves bookmarks, and confirms recursive deletion', () => {
  const bookmarkState: BrowserState = {
    ...state,
    bookmarks: [{ id: 'bookmark-one', title: 'Example', url: 'https://example.com', parentId: 'child', createdAt: 1 }],
    bookmarkFolders: [
      { id: 'parent', name: 'Parent', createdAt: 1 },
      { id: 'child', name: 'Child', parentId: 'parent', createdAt: 2 }
    ]
  }
  const confirm = vi.spyOn(window, 'confirm').mockReturnValue(true)
  render(<InternalPage url="archetype://bookmarks" state={bookmarkState} bridge={bridge} />)

  fireEvent.click(screen.getByRole('button', { name: 'Parent' }))
  fireEvent.click(screen.getByRole('button', { name: 'New folder' }))
  fireEvent.change(screen.getByRole('textbox', { name: 'Folder name' }), { target: { value: 'Grandchild' } })
  fireEvent.click(screen.getByRole('button', { name: 'Create' }))
  expect(bridge.createBookmarkFolder).toHaveBeenCalledWith('Grandchild', 'parent')

  fireEvent.click(screen.getByRole('button', { name: 'Child' }))
  fireEvent.change(screen.getByRole('combobox', { name: 'Move Example to folder' }), { target: { value: '' } })
  expect(bridge.moveBookmark).toHaveBeenCalledWith('bookmark-one', undefined)
  fireEvent.click(screen.getByRole('button', { name: 'Delete folder Child' }))
  expect(confirm).toHaveBeenCalledWith('Delete "Child" and all folders and bookmarks inside it?')
  expect(bridge.removeBookmarkFolder).toHaveBeenCalledWith('child')
  confirm.mockRestore()
})

it('opens the root folder form from the bookmark-bar shortcut', () => {
  render(<InternalPage url="archetype://bookmarks/new-folder" state={state} bridge={bridge} />)
  expect(screen.getByRole('textbox', { name: 'Folder name' })).toBeInTheDocument()
  expect(screen.getByRole('heading', { name: 'All bookmarks' })).toBeInTheDocument()
})
