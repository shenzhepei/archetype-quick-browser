import { contextBridge, ipcRenderer } from 'electron'
import type { ArchetypeBridge, BrowserSettings, BrowserState, ContentBounds } from '../shared/browser'

const bridge: ArchetypeBridge = {
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
  updateSettings: (settings: Partial<BrowserSettings>) =>
    ipcRenderer.invoke('browser:update-settings', settings),
  clearHistory: () => ipcRenderer.invoke('browser:clear-history'),
  setContentBounds: (bounds: ContentBounds) => ipcRenderer.send('browser:set-content-bounds', bounds),
  onState: (callback: (state: BrowserState) => void) => {
    const listener = (_event: Electron.IpcRendererEvent, state: BrowserState): void => callback(state)
    ipcRenderer.on('browser:state', listener)
    return () => ipcRenderer.removeListener('browser:state', listener)
  }
}

contextBridge.exposeInMainWorld('archetype', bridge)
