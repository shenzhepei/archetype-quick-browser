import { contextBridge, ipcRenderer } from 'electron'
import type { BrowserState, ShellBridge } from '../shared.js'

const bridge: ShellBridge = {
  getState: () => ipcRenderer.invoke('browser:get-state'),
  newTab: (url) => ipcRenderer.invoke('browser:new-tab', url),
  closeTab: (id) => ipcRenderer.invoke('browser:close-tab', id),
  selectTab: (id) => ipcRenderer.invoke('browser:select-tab', id),
  navigate: (input) => ipcRenderer.invoke('browser:navigate', input),
  back: () => ipcRenderer.invoke('browser:back'),
  forward: () => ipcRenderer.invoke('browser:forward'),
  reload: () => ipcRenderer.invoke('browser:reload'),
  updatePreferences: (value) => ipcRenderer.invoke('browser:update-preferences', value),
  setContentBounds: (bounds) => ipcRenderer.send('browser:set-content-bounds', bounds),
  onState: (listener) => {
    const handler = (_event: Electron.IpcRendererEvent, state: BrowserState): void => listener(state)
    ipcRenderer.on('browser:state', handler)
    return () => ipcRenderer.removeListener('browser:state', handler)
  }
}

contextBridge.exposeInMainWorld('archetypeShell', bridge)
