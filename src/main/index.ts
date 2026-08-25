import { join } from 'node:path'
import { app, BaseWindow, ipcMain, Menu, nativeImage, nativeTheme, WebContentsView } from 'electron'
import { BrowserController, configureSession } from './controller'
import { ReleaseService } from './release'
import { BrowserStore } from './store'
import type { BrowserSettings, ContentBounds, PopupPosition, TabMenuRequest } from '../shared/browser'

let mainWindow: BaseWindow | undefined
let shellView: WebContentsView | undefined
let controller: BrowserController | undefined
const releaseService = new ReleaseService(() => app.getVersion())

function updateWindowsTitleBar(): void {
  if (process.platform !== 'win32' || !mainWindow) return
  mainWindow.setTitleBarOverlay({
    color: nativeTheme.shouldUseDarkColors ? '#292b2f' : '#e8eaed',
    symbolColor: nativeTheme.shouldUseDarkColors ? '#eef0f3' : '#202124',
    height: 40
  })
}

const menuLabels = {
  en: { history: 'History', settings: 'Settings', reload: 'Reload', close: 'Close', closeOthers: 'Close other tabs', closeRight: 'Close tabs to the right' },
  'zh-CN': { history: '历史记录', settings: '设置', reload: '重新加载', close: '关闭', closeOthers: '关闭其他标签页', closeRight: '关闭右侧标签页' }
} as const

const menuLabel = (label: string): string => `${label}${'\u2002'.repeat(8)}`

function menuIcon(paths: string): Electron.NativeImage {
  const color = nativeTheme.shouldUseDarkColors ? '#eef0f3' : '#202124'
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="${color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${paths}</svg>`
  const icon = nativeImage.createFromDataURL(`data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}`)
  icon.setTemplateImage(true)
  return icon
}

function showBrowserMenu(position: PopupPosition): void {
  if (!mainWindow || !controller || !Number.isFinite(position.x) || !Number.isFinite(position.y)) return
  const language = controller.state().settings.language
  const labels = menuLabels[language]
  const bounds = mainWindow.getContentBounds()
  const x = Math.max(0, Math.min(Math.round(position.x), bounds.width))
  const y = Math.max(0, Math.min(Math.round(position.y), bounds.height))

  Menu.buildFromTemplate([
    {
      label: menuLabel(labels.history),
      icon: menuIcon('<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l4 2"/>'),
      click: () => controller?.openUtilityPage('history')
    },
    {
      label: menuLabel(labels.settings),
      icon: menuIcon('<path d="M4 21v-7"/><path d="M4 10V3"/><path d="M12 21v-9"/><path d="M12 8V3"/><path d="M20 21v-5"/><path d="M20 12V3"/><path d="M1 14h6"/><path d="M9 8h6"/><path d="M17 16h6"/>'),
      click: () => controller?.openUtilityPage('settings/appearance')
    }
  ]).popup({ window: mainWindow, x, y })
}

function showTabMenu(request: TabMenuRequest): void {
  if (!mainWindow || !controller || !Number.isFinite(request.x) || !Number.isFinite(request.y)) return
  const state = controller.state()
  const index = state.tabs.findIndex((tab) => tab.id === request.tabId)
  if (index < 0) return
  const labels = menuLabels[state.settings.language]
  const bounds = mainWindow.getContentBounds()
  const x = Math.max(0, Math.min(Math.round(request.x), bounds.width))
  const y = Math.max(0, Math.min(Math.round(request.y), bounds.height))
  Menu.buildFromTemplate([
    { label: labels.reload, click: () => controller?.reloadTab(request.tabId) },
    { type: 'separator' },
    { label: labels.close, click: () => controller?.closeTab(request.tabId) },
    { label: labels.closeOthers, enabled: state.tabs.length > 1, click: () => controller?.closeOtherTabs(request.tabId) },
    { label: labels.closeRight, enabled: index < state.tabs.length - 1, click: () => controller?.closeTabsToRight(request.tabId) }
  ]).popup({ window: mainWindow, x, y })
}

async function createWindow(): Promise<void> {
  const windowChrome =
    process.platform === 'darwin'
      ? {
          titleBarStyle: 'hiddenInset' as const,
          trafficLightPosition: { x: 16, y: 16 }
        }
      : process.platform === 'win32'
        ? {
            titleBarStyle: 'hidden' as const,
            titleBarOverlay: {
              color: '#e8eaed',
              symbolColor: '#202124',
              height: 40
            }
          }
        : {}

  mainWindow = new BaseWindow({
    width: 1280,
    height: 820,
    minWidth: 760,
    minHeight: 520,
    title: 'Archetype',
    backgroundColor: '#f4f5f7',
    ...windowChrome
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
  nativeTheme.themeSource = controller.state().settings.theme
  updateWindowsTitleBar()
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
  nativeTheme.on('updated', updateWindowsTitleBar)
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
  ipcMain.handle('browser:open-utility', (_event, path: 'history' | 'settings/appearance') => controller?.openUtilityPage(path))
  ipcMain.handle('browser:update-settings', (_event, settings: Partial<BrowserSettings>) => {
    controller?.updateSettings(settings)
    if (settings.theme) {
      nativeTheme.themeSource = settings.theme
      updateWindowsTitleBar()
    }
  })
  ipcMain.handle('browser:clear-history', () => controller?.clearHistory())
  ipcMain.handle('browser:show-menu', (_event, position: PopupPosition) => showBrowserMenu(position))
  ipcMain.handle('browser:show-tab-menu', (_event, request: TabMenuRequest) => showTabMenu(request))
  ipcMain.handle('browser:get-app-version', () => app.getVersion())
  ipcMain.handle('browser:check-for-updates', (_event, force?: boolean) => releaseService.check(force === true))
  ipcMain.handle('browser:open-latest-release', () => releaseService.openLatest())
  ipcMain.on('browser:set-content-bounds', (_event, bounds: ContentBounds) => controller?.setBounds(bounds))
}
