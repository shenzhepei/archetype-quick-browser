import { contextBridge, ipcRenderer } from 'electron'
import type { ArchetypeBridge, BookmarksOverflowRequest, BrowserSettings, BrowserState, ContentBounds, PopupPosition, TabMenuRequest } from '../shared/browser'

const bridge: ArchetypeBridge = {
  platform:
    process.platform === 'darwin' || process.platform === 'win32' ? process.platform : 'linux',
  getState: () => ipcRenderer.invoke('browser:get-state'),
  newTab: (url) => ipcRenderer.invoke('browser:new-tab', url),
  selectTab: (id) => ipcRenderer.invoke('browser:select-tab', id),
  closeTab: (id) => ipcRenderer.invoke('browser:close-tab', id),
  navigate: (input) => ipcRenderer.invoke('browser:navigate', input),
  back: () => ipcRenderer.invoke('browser:back'),
  forward: () => ipcRenderer.invoke('browser:forward'),
  reload: () => ipcRenderer.invoke('browser:reload'),
  stop: () => ipcRenderer.invoke('browser:stop'),
  toggleBookmark: () => ipcRenderer.invoke('browser:toggle-bookmark'),
  openInternal: (path) => ipcRenderer.invoke('browser:open-internal', path),
  openUtility: (path) => ipcRenderer.invoke('browser:open-utility', path),
  updateSettings: (settings: Partial<BrowserSettings>) =>
    ipcRenderer.invoke('browser:update-settings', settings),
  clearHistory: () => ipcRenderer.invoke('browser:clear-history'),
  removeBookmark: (id) => ipcRenderer.invoke('browser:remove-bookmark', id),
  createBookmarkFolder: (name, parentId) => ipcRenderer.invoke('browser:create-bookmark-folder', name, parentId),
  removeBookmarkFolder: (id) => ipcRenderer.invoke('browser:remove-bookmark-folder', id),
  moveBookmark: (id, parentId) => ipcRenderer.invoke('browser:move-bookmark', id, parentId),
  showMenu: (position: PopupPosition) => ipcRenderer.invoke('browser:show-menu', position),
  showTabMenu: (request: TabMenuRequest) => ipcRenderer.invoke('browser:show-tab-menu', request),
  showBookmarksBarMenu: (position: PopupPosition) => ipcRenderer.invoke('browser:show-bookmarks-bar-menu', position),
  showBookmarksOverflowMenu: (request: BookmarksOverflowRequest) => ipcRenderer.invoke('browser:show-bookmarks-overflow-menu', request),
  showSiteInfo: (position: PopupPosition) => ipcRenderer.invoke('browser:show-site-info', position),
  listExtensions: () => ipcRenderer.invoke('browser:list-extensions'),
  installExtension: () => ipcRenderer.invoke('browser:install-extension'),
  removeExtension: (id) => ipcRenderer.invoke('browser:remove-extension', id),
  getAppVersion: () => ipcRenderer.invoke('browser:get-app-version'),
  checkForUpdates: (force = false) => ipcRenderer.invoke('browser:check-for-updates', force),
  openLatestRelease: () => ipcRenderer.invoke('browser:open-latest-release'),
  setContentBounds: (bounds: ContentBounds) => ipcRenderer.send('browser:set-content-bounds', bounds),
  onState: (callback: (state: BrowserState) => void) => {
    const listener = (_event: Electron.IpcRendererEvent, state: BrowserState): void => callback(state)
    ipcRenderer.on('browser:state', listener)
    return () => ipcRenderer.removeListener('browser:state', listener)
  }
}

contextBridge.exposeInMainWorld('archetype', bridge)
