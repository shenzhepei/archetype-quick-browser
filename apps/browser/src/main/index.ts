import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'
import { app, BaseWindow, ipcMain, WebContentsView } from 'electron'
import type { Language, Theme } from '../shared.js'
import { BrowserController } from './controller.js'
import { RuntimeClient } from './runtime-client.js'

const currentDirectory = dirname(fileURLToPath(import.meta.url))
let browserController: BrowserController | undefined

function requireShell(event: Electron.IpcMainInvokeEvent, shell: Electron.WebContents): void {
  if (event.sender !== shell) throw new Error('Browser command rejected from an untrusted renderer.')
}

function pageUrl(event: Electron.IpcMainInvokeEvent): string {
  const frame = event.senderFrame
  if (!frame || frame.parent !== null) throw Object.assign(new Error('Runtime API is limited to top-level frames.'), { name: 'SecurityError' })
  return frame.url
}

async function createWindow(): Promise<void> {
  const window = new BaseWindow({ width: 1360, height: 860, minWidth: 780, minHeight: 560, title: 'Archetype Runtime' })
  const shellView = new WebContentsView({ webPreferences: { preload: join(currentDirectory, '../preload/index.cjs'), sandbox: true, contextIsolation: true, nodeIntegration: false } })
  window.contentView.addChildView(shellView)
  const resizeShell = (): void => {
    const bounds = window.getBounds()
    shellView.setBounds({ x: 0, y: 0, width: bounds.width, height: bounds.height })
  }
  resizeShell()
  window.on('resize', resizeShell)
  if (process.env.ELECTRON_RENDERER_URL) await shellView.webContents.loadURL(process.env.ELECTRON_RENDERER_URL)
  else await shellView.webContents.loadFile(join(currentDirectory, '../renderer/index.html'))

  const runtime = new RuntimeClient()
  const subscriptions = new Map<string, () => void>()
  await runtime.initialize()
  const controller = new BrowserController(window, shellView.webContents, join(currentDirectory, '../preload/runtime-page.cjs'), runtime)
  browserController = controller

  ipcMain.handle('browser:get-state', (event) => { requireShell(event, shellView.webContents); return controller.state() })
  ipcMain.handle('browser:new-tab', (event, url?: string) => { requireShell(event, shellView.webContents); controller.createTab(url) })
  ipcMain.handle('browser:close-tab', (event, id: string) => { requireShell(event, shellView.webContents); controller.closeTab(id) })
  ipcMain.handle('browser:select-tab', (event, id: string) => { requireShell(event, shellView.webContents); controller.selectTab(id) })
  ipcMain.handle('browser:navigate', (event, input: string) => { requireShell(event, shellView.webContents); controller.navigate(input) })
  ipcMain.handle('browser:back', (event) => { requireShell(event, shellView.webContents); controller.back() })
  ipcMain.handle('browser:forward', (event) => { requireShell(event, shellView.webContents); controller.forward() })
  ipcMain.handle('browser:reload', (event) => { requireShell(event, shellView.webContents); controller.reload() })
  ipcMain.handle('browser:update-preferences', (event, value: { language?: Language; theme?: Theme }) => { requireShell(event, shellView.webContents); controller.updatePreferences(value) })
  ipcMain.on('browser:set-content-bounds', (event, bounds) => { if (event.sender === shellView.webContents) controller.setBounds(bounds) })

  ipcMain.handle('runtime:request', async (event, request: { action: string; requestId?: string; operation?: string; input?: unknown; options?: { idempotencyKey?: string; timeoutMs?: number } }) => {
    try {
      const url = pageUrl(event)
      if (request.action === 'discover') return { ok: true, value: await runtime.discover(url) }
      if (request.action === 'signIn') return { ok: true, value: await runtime.signIn(url) }
      if (request.action === 'signOut') return { ok: true, value: await runtime.signOut(url) }
      if (request.action === 'session') return { ok: true, value: await runtime.session(url) }
      if (request.action === 'invoke' && request.requestId && request.operation) {
        const value = await runtime.invoke(url, request.requestId, request.operation, request.input, request.options ?? {})
        event.sender.send('runtime:event', { topic: `${request.operation}.completed`, payload: value })
        return { ok: true, value }
      }
      throw Object.assign(new Error('Unsupported Runtime request.'), { name: 'NotSupportedError' })
    } catch (error) {
      return { ok: false, error: { name: error instanceof Error ? error.name : 'OperationError', message: error instanceof Error ? error.message : 'Runtime request failed.' } }
    }
  })
  ipcMain.on('runtime:cancel', (event, requestId: string) => { pageUrl(event as unknown as Electron.IpcMainInvokeEvent); runtime.cancel(requestId) })
  ipcMain.on('runtime:subscribe', (event, topic: string) => {
    try {
      const url = pageUrl(event as unknown as Electron.IpcMainInvokeEvent)
      const key = `${event.sender.id}:${topic}`
      subscriptions.get(key)?.()
      subscriptions.set(key, runtime.subscribe(url, topic, (payload) => {
        if (!event.sender.isDestroyed() && event.sender.getURL() === url) event.sender.send('runtime:event', { topic, payload })
      }))
    } catch {
      // subscribe() has no error callback; invalid or ineligible contexts receive no events.
    }
  })
  ipcMain.on('runtime:unsubscribe', (event, topic: string) => {
    const key = `${event.sender.id}:${topic}`
    subscriptions.get(key)?.()
    subscriptions.delete(key)
  })

  const clearSubscriptions = (contents: Electron.WebContents): void => {
    for (const [key, cancel] of subscriptions) {
      if (key.startsWith(`${contents.id}:`)) {
        cancel()
        subscriptions.delete(key)
      }
    }
  }
  app.on('web-contents-created', (_event, contents) => {
    contents.on('did-start-navigation', (_navigationEvent, _url, _isInPlace, isMainFrame) => { if (isMainFrame) clearSubscriptions(contents) })
    contents.once('destroyed', () => clearSubscriptions(contents))
  })

  controller.initialize()
  window.on('closed', () => {
    for (const cancel of subscriptions.values()) cancel()
    subscriptions.clear()
    if (browserController === controller) browserController = undefined
  })
}

app.whenReady().then(async () => {
  await createWindow()
  app.on('activate', () => { if (!browserController) void createWindow() })
})

app.on('window-all-closed', () => { if (process.platform !== 'darwin') app.quit() })
