import { join } from 'node:path'
import { app, BaseWindow, dialog, ipcMain, Menu, nativeImage, nativeTheme, session, WebContentsView } from 'electron'
import { BrowserController } from './controller'
import { ReleaseService } from './release'
import { SiteSecurityService } from './site-security'
import { BrowserStore } from './store'
import type { BrowserSettings, ContentBounds, PopupPosition, TabMenuRequest } from '../shared/browser'

let mainWindow: BaseWindow | undefined
let shellView: WebContentsView | undefined
let controller: BrowserController | undefined
const releaseService = new ReleaseService(() => app.getVersion())
const siteSecurity = new SiteSecurityService()

function updateWindowsTitleBar(): void {
  if (process.platform !== 'win32' || !mainWindow) return
  mainWindow.setTitleBarOverlay({
    color: nativeTheme.shouldUseDarkColors ? '#292b2f' : '#e8eaed',
    symbolColor: nativeTheme.shouldUseDarkColors ? '#eef0f3' : '#202124',
    height: 40
  })
}

const menuLabels = {
  en: { history: 'History', settings: 'Settings', reload: 'Reload', close: 'Close', closeOthers: 'Close other tabs', closeRight: 'Close tabs to the right', secure: 'Connection is secure', verifying: 'Checking secure connection', insecure: 'Connection is not secure', local: 'Local page', internal: 'Archetype internal page', noSite: 'No site information', certificate: 'Certificate details', permissions: 'Permissions', noPermissions: 'No permissions granted', blocked: 'Blocked', granted: 'Allowed', subject: 'Subject', issuer: 'Issuer', validFrom: 'Valid from', validUntil: 'Valid until', fingerprint: 'SHA-256 fingerprint', knownRoot: 'Issued by a known root', verification: 'Chromium verification', yes: 'Yes', no: 'No' },
  'zh-CN': { history: '历史记录', settings: '设置', reload: '重新加载', close: '关闭', closeOthers: '关闭其他标签页', closeRight: '关闭右侧标签页', secure: '连接安全', verifying: '正在验证安全连接', insecure: '连接不安全', local: '本地页面', internal: 'Archetype 内部页面', noSite: '没有站点信息', certificate: '证书信息', permissions: '权限', noPermissions: '没有已授权权限', blocked: '已阻止', granted: '已允许', subject: '使用者', issuer: '颁发者', validFrom: '生效时间', validUntil: '到期时间', fingerprint: 'SHA-256 指纹', knownRoot: '已知根证书颁发', verification: 'Chromium 验证结果', yes: '是', no: '否' }
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

function showSiteInfo(position: PopupPosition): void {
  if (!mainWindow || !controller || !Number.isFinite(position.x) || !Number.isFinite(position.y)) return
  const state = controller.state()
  const info = state.siteInfo
  const labels = menuLabels[state.settings.language]
  const connectionLabels = {
    secure: labels.secure,
    verifying: labels.verifying,
    insecure: labels.insecure,
    local: labels.local,
    internal: labels.internal,
    none: labels.noSite
  }
  const permissionLabels: Record<string, { en: string; 'zh-CN': string }> = {
    media: { en: 'Camera and microphone', 'zh-CN': '摄像头和麦克风' },
    geolocation: { en: 'Location', 'zh-CN': '位置信息' },
    notifications: { en: 'Notifications', 'zh-CN': '通知' },
    'clipboard-read': { en: 'Clipboard', 'zh-CN': '剪贴板' },
    midi: { en: 'MIDI devices', 'zh-CN': 'MIDI 设备' },
    fullscreen: { en: 'Fullscreen', 'zh-CN': '全屏' }
  }
  const iconPaths = info.connection === 'secure'
    ? '<rect width="18" height="11" x="3" y="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>'
    : '<path d="m21.7 16-8-14a2 2 0 0 0-3.4 0l-8 14A2 2 0 0 0 4 19h16a2 2 0 0 0 1.7-3Z"/><path d="M12 9v4"/><path d="M12 17h.01"/>'
  const template: Electron.MenuItemConstructorOptions[] = [
    { label: menuLabel(connectionLabels[info.connection]), icon: menuIcon(iconPaths), enabled: false }
  ]
  if (info.certificate) {
    template.push({
      label: labels.certificate,
      click: () => showCertificateDetails(info.certificate!, state.settings.language)
    })
  }
  template.push({ type: 'separator' }, { label: labels.permissions, enabled: false })
  if (info.permissions.length === 0) {
    template.push({ label: labels.noPermissions, enabled: false })
  } else {
    for (const permission of info.permissions) {
      const name = permissionLabels[permission.permission]?.[state.settings.language] ?? permission.permission
      template.push({ label: `${name}: ${permission.state === 'granted' ? labels.granted : labels.blocked}`, enabled: false })
    }
  }
  const bounds = mainWindow.getContentBounds()
  const x = Math.max(0, Math.min(Math.round(position.x), bounds.width))
  const y = Math.max(0, Math.min(Math.round(position.y), bounds.height))
  Menu.buildFromTemplate(template).popup({ window: mainWindow, x, y })
}

function showCertificateDetails(certificate: NonNullable<ReturnType<BrowserController['state']>['siteInfo']['certificate']>, language: 'en' | 'zh-CN'): void {
  if (!mainWindow) return
  const labels = menuLabels[language]
  const locale = language === 'zh-CN' ? 'zh-CN' : 'en-US'
  void dialog.showMessageBox(mainWindow, {
    type: 'info',
    title: labels.certificate,
    message: certificate.subjectName,
    detail: [
      `${labels.subject}: ${certificate.subjectName}`,
      `${labels.issuer}: ${certificate.issuerName}`,
      `${labels.validFrom}: ${new Date(certificate.validStart * 1000).toLocaleString(locale)}`,
      `${labels.validUntil}: ${new Date(certificate.validExpiry * 1000).toLocaleString(locale)}`,
      `${labels.fingerprint}: ${certificate.fingerprint}`,
      `${labels.knownRoot}: ${certificate.isIssuedByKnownRoot ? labels.yes : labels.no}`,
      `${labels.verification}: ${certificate.verificationResult} (${certificate.errorCode})`
    ].join('\n')
  })
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

  controller = new BrowserController(mainWindow, shellView.webContents, new BrowserStore(), siteSecurity)
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
  siteSecurity.configure(session.fromPartition('persist:archetype'), () => controller?.refreshSiteInfo())
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
  ipcMain.handle('browser:show-site-info', (_event, position: PopupPosition) => showSiteInfo(position))
  ipcMain.handle('browser:get-app-version', () => app.getVersion())
  ipcMain.handle('browser:check-for-updates', (_event, force?: boolean) => releaseService.check(force === true))
  ipcMain.handle('browser:open-latest-release', () => releaseService.openLatest())
  ipcMain.on('browser:set-content-bounds', (_event, bounds: ContentBounds) => controller?.setBounds(bounds))
}
