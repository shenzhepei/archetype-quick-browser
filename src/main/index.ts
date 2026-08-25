import { join } from 'node:path'
import { app, BaseWindow, ipcMain, nativeTheme, WebContentsView } from 'electron'
import { BrowserController, configureSession } from './controller'
import { BrowserStore } from './store'
import type { BrowserSettings, ContentBounds } from '../shared/browser'

let mainWindow: BaseWindow | undefined
let shellView: WebContentsView | undefined
let controller: BrowserController | undefined

async function createWindow(): Promise<void> {
  mainWindow = new BaseWindow({
    width: 1280,
    height: 820,
    minWidth: 760,
    minHeight: 520,
    title: 'Archetype',
    titleBarStyle: 'hiddenInset',
    trafficLightPosition: { x: 16, y: 16 },
    backgroundColor: '#f4f5f7'
  })
  shellView = new WebContentsView({
    webPreferences: {
      preload: join(__dirname, '../preload/index.cjs'),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false
    }
  })
  mainWindow.contentView.addChildView(shellView)
  const layoutShell = (): void => {
    const { width, height } = mainWindow!.getContentBounds()
    shellView!.setBounds({ x: 0, y: 0, width, height })
  }
  layoutShell()
  mainWindow.on('resize', layoutShell)

  controller = new BrowserController(mainWindow, shellView.webContents, new BrowserStore())
  if (process.env.ELECTRON_RENDERER_URL) await shellView.webContents.loadURL(process.env.ELECTRON_RENDERER_URL)
  else await shellView.webContents.loadFile(join(__dirname, '../renderer/index.html'))
  await controller.initialize()
  mainWindow.on('closed', () => {
    controller?.dispose()
    shellView?.webContents.close()
    controller = undefined
    shellView = undefined
    mainWindow = undefined
  })
}

app.whenReady().then(async () => {
  configureSession()
  registerIpc()
  await createWindow()
  app.on('activate', () => {
    if (!mainWindow) void createWindow()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})

function registerIpc(): void {
  ipcMain.handle('browser:get-state', () => controller?.state())
  ipcMain.handle('browser:new-tab', (_event, url?: string) => controller?.createTab(url))
  ipcMain.handle('browser:select-tab', (_event, id: string) => controller?.selectTab(id))
  ipcMain.handle('browser:close-tab', (_event, id: string) => controller?.closeTab(id))
  ipcMain.handle('browser:navigate', (_event, input: string) => controller?.navigate(input))
  ipcMain.handle('browser:back', () => controller?.back())
  ipcMain.handle('browser:forward', () => controller?.forward())
  ipcMain.handle('browser:reload', () => controller?.reload())
  ipcMain.handle('browser:stop', () => controller?.stop())
  ipcMain.handle('browser:toggle-bookmark', () => controller?.toggleBookmark())
  ipcMain.handle('browser:open-internal', (_event, path: string) => controller?.navigate(`archetype://${path}`))
  ipcMain.handle('browser:update-settings', (_event, settings: Partial<BrowserSettings>) => {
    controller?.updateSettings(settings)
    if (settings.theme) nativeTheme.themeSource = settings.theme
  })
  ipcMain.handle('browser:clear-history', () => controller?.clearHistory())
  ipcMain.on('browser:set-content-bounds', (_event, bounds: ContentBounds) => controller?.setBounds(bounds))
}
