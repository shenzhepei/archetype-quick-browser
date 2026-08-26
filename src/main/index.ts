import { join } from 'node:path'
import { app, BaseWindow, dialog, ipcMain, Menu, nativeImage, nativeTheme, session, WebContentsView } from 'electron'
import { BrowserController } from './controller'
import { ExtensionService } from './extension-service'
import { compactMenuTitle, recentBookmarks, recentHistory } from './recent-menu'
import { ReleaseService } from './release'
import { SiteSecurityService } from './site-security'
import { BrowserStore } from './store'
import { buildBookmarkTree } from '../shared/bookmark-tree'
import type { BookmarkFolderNode } from '../shared/bookmark-tree'
import type { BookmarksOverflowRequest, BrowserSettings, ContentBounds, PopupPosition, TabMenuRequest } from '../shared/browser'

interface WindowContext {
  window: BaseWindow
  shellView: WebContentsView
  controller: BrowserController
}

const windowContexts = new Map<number, WindowContext>()
const browserStore = new BrowserStore()
const releaseService = new ReleaseService(() => app.getVersion())
const siteSecurity = new SiteSecurityService()
let extensionService: ExtensionService

function updateWindowsTitleBar(): void {
  if (process.platform !== 'win32') return
  for (const { window } of windowContexts.values()) {
    window.setTitleBarOverlay({
      color: nativeTheme.shouldUseDarkColors ? '#292b2f' : '#e8eaed',
      symbolColor: nativeTheme.shouldUseDarkColors ? '#eef0f3' : '#202124',
      height: 40
    })
  }
}

const menuLabels = {
  en: { newTab: 'Open new tab', newWindow: 'Open new window', extensions: 'Extensions', manageExtensions: 'Manage extensions', history: 'History', showFullHistory: 'Show full history', noRecentHistory: 'No recent history', bookmarks: 'Bookmarks', bookmarkThisTab: 'Bookmark this tab', removeCurrentBookmark: 'Remove bookmark for this tab', showAllBookmarks: 'Show all bookmarks', recentBookmarks: 'Recent bookmarks', noBookmarks: 'No bookmarks', emptyFolder: 'Empty folder', addPage: 'Add page', addFolder: 'Add folder', openBookmarkManager: 'Open bookmark manager', moreBookmarks: 'More bookmarks', print: 'Print', settings: 'Settings', reload: 'Reload', close: 'Close', closeOthers: 'Close other tabs', closeRight: 'Close tabs to the right', secure: 'Connection is secure', verifying: 'Checking secure connection', insecure: 'Connection is not secure', local: 'Local page', internal: 'Archetype internal page', noSite: 'No site information', certificate: 'Certificate details', permissions: 'Permissions', noPermissions: 'No permissions granted', blocked: 'Blocked', granted: 'Allowed', subject: 'Subject', issuer: 'Issuer', validFrom: 'Valid from', validUntil: 'Valid until', fingerprint: 'SHA-256 fingerprint', knownRoot: 'Issued by a known root', verification: 'Chromium verification', yes: 'Yes', no: 'No' },
  'zh-CN': { newTab: '打开新的标签页', newWindow: '打开新的窗口', extensions: '扩展程序', manageExtensions: '管理扩展程序', history: '历史记录', showFullHistory: '显示完整历史记录', noRecentHistory: '没有最近历史记录', bookmarks: '书签', bookmarkThisTab: '为此标签页添加书签', removeCurrentBookmark: '移除此标签页的书签', showAllBookmarks: '显示所有书签', recentBookmarks: '最近添加的书签', noBookmarks: '没有书签', emptyFolder: '空文件夹', addPage: '添加网页', addFolder: '添加文件夹', openBookmarkManager: '打开书签管理器', moreBookmarks: '更多书签', print: '打印', settings: '设置', reload: '重新加载', close: '关闭', closeOthers: '关闭其他标签页', closeRight: '关闭右侧标签页', secure: '连接安全', verifying: '正在验证安全连接', insecure: '连接不安全', local: '本地页面', internal: 'Archetype 内部页面', noSite: '没有站点信息', certificate: '证书信息', permissions: '权限', noPermissions: '没有已授权权限', blocked: '已阻止', granted: '已允许', subject: '使用者', issuer: '颁发者', validFrom: '生效时间', validUntil: '到期时间', fingerprint: 'SHA-256 指纹', knownRoot: '已知根证书颁发', verification: 'Chromium 验证结果', yes: '是', no: '否' }
} as const

const menuLabel = (label: string): string => `${label}${'\u2002'.repeat(8)}`

function menuIcon(paths: string): Electron.NativeImage {
  const color = nativeTheme.shouldUseDarkColors ? '#eef0f3' : '#202124'
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="${color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${paths}</svg>`
  const icon = nativeImage.createFromDataURL(`data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}`)
  icon.setTemplateImage(true)
  return icon
}

function bookmarkMenuIcon(favicon?: string): Electron.NativeImage | undefined {
  if (!favicon?.startsWith('data:image/')) return undefined
  const icon = nativeImage.createFromDataURL(favicon)
  return icon.isEmpty() ? undefined : icon.resize({ width: 16, height: 16 })
}

function bookmarkMenuItems(
  folders: BookmarkFolderNode[],
  bookmarks: BookmarkFolderNode['bookmarks'],
  controller: BrowserController,
  emptyLabel: string
): Electron.MenuItemConstructorOptions[] {
  const items: Electron.MenuItemConstructorOptions[] = [
    ...folders.map((folder) => ({
      label: folder.name,
      submenu: bookmarkMenuItems(folder.folders, folder.bookmarks, controller, emptyLabel)
    })),
    ...bookmarks.map((bookmark) => ({
      label: compactMenuTitle(bookmark.title, bookmark.url),
      icon: bookmarkMenuIcon(bookmark.favicon),
      toolTip: bookmark.url,
      click: () => controller.createTab(bookmark.url)
    }))
  ]
  return items.length > 0 ? items : [{ label: emptyLabel, enabled: false }]
}

function showBrowserMenu(context: WindowContext, position: PopupPosition): void {
  if (!Number.isFinite(position.x) || !Number.isFinite(position.y)) return
  const { window, controller } = context
  const state = controller.state()
  const language = state.settings.language
  const labels = menuLabels[language]
  const bounds = window.getContentBounds()
  const x = Math.max(0, Math.min(Math.round(position.x), bounds.width))
  const y = Math.max(0, Math.min(Math.round(position.y), bounds.height))
  const active = state.tabs.find((tab) => tab.id === state.activeTabId)
  const bookmarkable = Boolean(active && /^(https?|file):/i.test(active.url))
  const currentBookmark = active ? state.bookmarks.find((bookmark) => bookmark.url === active.url) : undefined
  const historyItems = recentHistory(state.history)
  const bookmarkItems = recentBookmarks(state.bookmarks)
  const bookmarkTree = buildBookmarkTree(state.bookmarks, state.bookmarkFolders)
  const historySubmenu: Electron.MenuItemConstructorOptions[] = [
    { label: labels.showFullHistory, click: () => controller.openUtilityPage('history') },
    { type: 'separator' },
    ...(historyItems.length > 0
      ? historyItems.map((entry) => ({
          label: compactMenuTitle(entry.title, entry.url),
          toolTip: entry.url,
          click: () => controller.createTab(entry.url)
        }))
      : [{ label: labels.noRecentHistory, enabled: false }])
  ]
  const bookmarkSubmenu: Electron.MenuItemConstructorOptions[] = [
    {
      label: currentBookmark ? labels.removeCurrentBookmark : labels.bookmarkThisTab,
      enabled: bookmarkable,
      click: () => controller.toggleBookmark()
    },
    { label: labels.showAllBookmarks, click: () => controller.openUtilityPage('bookmarks') },
    { type: 'separator' },
    {
      label: labels.recentBookmarks,
      submenu: bookmarkItems.length > 0
        ? bookmarkItems.map((bookmark) => ({
            label: compactMenuTitle(bookmark.title, bookmark.url),
            icon: bookmarkMenuIcon(bookmark.favicon),
            toolTip: bookmark.url,
            click: () => controller.createTab(bookmark.url)
          }))
        : [{ label: labels.noBookmarks, enabled: false }]
    },
    { type: 'separator' },
    ...bookmarkMenuItems(bookmarkTree.folders, bookmarkTree.bookmarks, controller, labels.emptyFolder)
  ]

  Menu.buildFromTemplate([
    {
      label: menuLabel(labels.newTab),
      icon: menuIcon('<path d="M12 5v14"/><path d="M5 12h14"/>'),
      click: () => controller.createTab()
    },
    {
      label: menuLabel(labels.newWindow),
      icon: menuIcon('<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/><path d="M9 9h12"/>'),
      click: () => void createWindow(false)
    },
    { type: 'separator' },
    {
      label: menuLabel(labels.history),
      icon: menuIcon('<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l4 2"/>'),
      submenu: historySubmenu
    },
    {
      label: menuLabel(labels.print),
      icon: menuIcon('<path d="M6 9V2h12v7"/><path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/><rect width="12" height="8" x="6" y="14"/>'),
      enabled: controller.canPrint(),
      click: () => controller.print()
    },
    {
      label: menuLabel(labels.bookmarks),
      icon: menuIcon('<path d="m12 2 3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01z"/>'),
      submenu: bookmarkSubmenu
    },
    {
      label: menuLabel(labels.extensions),
      icon: menuIcon('<path d="M15.39 4.39a1 1 0 0 0 1.68-.474 2.5 2.5 0 1 1 3.014 3.015 1 1 0 0 0-.474 1.68l1.683 1.682a2.414 2.414 0 0 1 0 3.414L19.61 15.39a1 1 0 0 1-1.68-.474 2.5 2.5 0 1 0-3.014 3.015 1 1 0 0 1 .474 1.68l-1.683 1.682a2.414 2.414 0 0 1-3.414 0L8.61 19.61a1 1 0 0 0-1.68.474 2.5 2.5 0 1 1-3.014-3.015 1 1 0 0 0 .474-1.68l-1.683-1.682a2.414 2.414 0 0 1 0-3.414L4.39 8.61a1 1 0 0 1 1.68.474 2.5 2.5 0 1 0 3.014-3.015 1 1 0 0 1-.474-1.68l1.683-1.682a2.414 2.414 0 0 1 3.414 0z"/>'),
      submenu: [{ label: labels.manageExtensions, click: () => controller.openUtilityPage('extensions') }]
    },
    {
      label: menuLabel(labels.settings),
      icon: menuIcon('<path d="M4 21v-7"/><path d="M4 10V3"/><path d="M12 21v-9"/><path d="M12 8V3"/><path d="M20 21v-5"/><path d="M20 12V3"/><path d="M1 14h6"/><path d="M9 8h6"/><path d="M17 16h6"/>'),
      click: () => controller.openUtilityPage('settings/appearance')
    }
  ]).popup({ window, x, y })
}

function showTabMenu(context: WindowContext, request: TabMenuRequest): void {
  if (!Number.isFinite(request.x) || !Number.isFinite(request.y)) return
  const { window, controller } = context
  const state = controller.state()
  const index = state.tabs.findIndex((tab) => tab.id === request.tabId)
  if (index < 0) return
  const labels = menuLabels[state.settings.language]
  const bounds = window.getContentBounds()
  const x = Math.max(0, Math.min(Math.round(request.x), bounds.width))
  const y = Math.max(0, Math.min(Math.round(request.y), bounds.height))
  Menu.buildFromTemplate([
    { label: labels.reload, click: () => controller.reloadTab(request.tabId) },
    { type: 'separator' },
    { label: labels.close, click: () => controller.closeTab(request.tabId) },
    { label: labels.closeOthers, enabled: state.tabs.length > 1, click: () => controller.closeOtherTabs(request.tabId) },
    { label: labels.closeRight, enabled: index < state.tabs.length - 1, click: () => controller.closeTabsToRight(request.tabId) }
  ]).popup({ window, x, y })
}

function popupPosition(window: BaseWindow, position: PopupPosition): { x: number; y: number } | undefined {
  if (!Number.isFinite(position.x) || !Number.isFinite(position.y)) return undefined
  const bounds = window.getContentBounds()
  return {
    x: Math.max(0, Math.min(Math.round(position.x), bounds.width)),
    y: Math.max(0, Math.min(Math.round(position.y), bounds.height))
  }
}

function showBookmarksBarMenu(context: WindowContext, position: PopupPosition): void {
  const popup = popupPosition(context.window, position)
  if (!popup) return
  const labels = menuLabels[context.controller.state().settings.language]
  Menu.buildFromTemplate([
    { label: labels.addPage, enabled: context.controller.canAddActivePageBookmark(), click: () => context.controller.addActivePageBookmark() },
    { label: labels.addFolder, click: () => context.controller.openBookmarkManager(true) },
    { type: 'separator' },
    { label: labels.openBookmarkManager, click: () => context.controller.openBookmarkManager() }
  ]).popup({ window: context.window, ...popup })
}

function showBookmarksOverflowMenu(context: WindowContext, request: BookmarksOverflowRequest): void {
  const popup = popupPosition(context.window, request)
  if (!popup || !Array.isArray(request.bookmarkIds)) return
  const state = context.controller.state()
  const requested = new Set(request.bookmarkIds.filter((id): id is string => typeof id === 'string').slice(0, 1000))
  const bookmarks = state.bookmarks.filter((bookmark) => requested.has(bookmark.id))
  const labels = menuLabels[state.settings.language]
  const template: Electron.MenuItemConstructorOptions[] = bookmarks.length > 0
    ? bookmarks.map((bookmark) => ({
        label: compactMenuTitle(bookmark.title, bookmark.url),
        icon: bookmarkMenuIcon(bookmark.favicon),
        toolTip: bookmark.url,
        click: () => context.controller.navigate(bookmark.url)
      }))
    : [{ label: labels.noBookmarks, enabled: false }]
  Menu.buildFromTemplate(template).popup({ window: context.window, ...popup })
}

function showSiteInfo(context: WindowContext, position: PopupPosition): void {
  if (!Number.isFinite(position.x) || !Number.isFinite(position.y)) return
  const { window, controller } = context
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
      click: () => showCertificateDetails(window, info.certificate!, state.settings.language)
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
  const bounds = window.getContentBounds()
  const x = Math.max(0, Math.min(Math.round(position.x), bounds.width))
  const y = Math.max(0, Math.min(Math.round(position.y), bounds.height))
  Menu.buildFromTemplate(template).popup({ window, x, y })
}

function showCertificateDetails(window: BaseWindow, certificate: NonNullable<ReturnType<BrowserController['state']>['siteInfo']['certificate']>, language: 'en' | 'zh-CN'): void {
  const labels = menuLabels[language]
  const locale = language === 'zh-CN' ? 'zh-CN' : 'en-US'
  void dialog.showMessageBox(window, {
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

async function createWindow(restoreTabs: boolean): Promise<void> {
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

  const window = new BaseWindow({
    width: 1280,
    height: 820,
    minWidth: 760,
    minHeight: 520,
    title: 'Archetype',
    backgroundColor: '#f4f5f7',
    ...windowChrome
  })
  const shellView = new WebContentsView({
    webPreferences: {
      preload: join(__dirname, '../preload/index.cjs'),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false
    }
  })
  window.contentView.addChildView(shellView)
  const layoutShell = (): void => {
    const { width, height } = window.getContentBounds()
    shellView.setBounds({ x: 0, y: 0, width, height })
  }
  layoutShell()
  window.on('resize', layoutShell)

  const controller = new BrowserController(window, shellView.webContents, browserStore, siteSecurity, restoreTabs)
  const context = { window, shellView, controller }
  windowContexts.set(shellView.webContents.id, context)
  if (process.env.ELECTRON_RENDERER_URL) await shellView.webContents.loadURL(process.env.ELECTRON_RENDERER_URL)
  else await shellView.webContents.loadFile(join(__dirname, '../renderer/index.html'))
  await controller.initialize()
  nativeTheme.themeSource = controller.state().settings.theme
  updateWindowsTitleBar()
  window.on('closed', () => {
    windowContexts.delete(shellView.webContents.id)
    controller.dispose()
    shellView.webContents.close()
  })
}

app.whenReady().then(async () => {
  await browserStore.load()
  const browserSession = session.fromPartition('persist:archetype')
  siteSecurity.configure(browserSession, () => {
    for (const { controller } of windowContexts.values()) controller.refreshSiteInfo()
  })
  extensionService = new ExtensionService(browserSession, browserStore)
  await extensionService.initialize()
  registerIpc()
  nativeTheme.on('updated', updateWindowsTitleBar)
  await createWindow(true)
  app.on('activate', () => {
    if (windowContexts.size === 0) void createWindow(true)
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})

function registerIpc(): void {
  const contextFor = (senderId: number): WindowContext | undefined => windowContexts.get(senderId)
  ipcMain.handle('browser:get-state', (event) => contextFor(event.sender.id)?.controller.state())
  ipcMain.handle('browser:new-tab', (event, url?: string) => contextFor(event.sender.id)?.controller.createTab(url))
  ipcMain.handle('browser:select-tab', (event, id: string) => contextFor(event.sender.id)?.controller.selectTab(id))
  ipcMain.handle('browser:close-tab', (event, id: string) => contextFor(event.sender.id)?.controller.closeTab(id))
  ipcMain.handle('browser:navigate', (event, input: string) => contextFor(event.sender.id)?.controller.navigate(input))
  ipcMain.handle('browser:back', (event) => contextFor(event.sender.id)?.controller.back())
  ipcMain.handle('browser:forward', (event) => contextFor(event.sender.id)?.controller.forward())
  ipcMain.handle('browser:reload', (event) => contextFor(event.sender.id)?.controller.reload())
  ipcMain.handle('browser:stop', (event) => contextFor(event.sender.id)?.controller.stop())
  ipcMain.handle('browser:toggle-bookmark', (event) => contextFor(event.sender.id)?.controller.toggleBookmark())
  ipcMain.handle('browser:open-internal', (event, path: string) => {
    const allowed = ['history', 'bookmarks', 'extensions', 'settings/appearance', 'settings/languages', 'settings/about']
    if (allowed.includes(path)) contextFor(event.sender.id)?.controller.navigate(`archetype://${path}`)
  })
  ipcMain.handle('browser:open-utility', (event, path: 'history' | 'bookmarks' | 'extensions' | 'settings/appearance') => {
    if (['history', 'bookmarks', 'extensions', 'settings/appearance'].includes(path)) {
      contextFor(event.sender.id)?.controller.openUtilityPage(path)
    }
  })
  ipcMain.handle('browser:update-settings', (event, settings: Partial<BrowserSettings>) => {
    contextFor(event.sender.id)?.controller.updateSettings(settings)
    if (settings.theme) {
      nativeTheme.themeSource = settings.theme
      updateWindowsTitleBar()
    }
  })
  ipcMain.handle('browser:clear-history', (event) => contextFor(event.sender.id)?.controller.clearHistory())
  ipcMain.handle('browser:remove-bookmark', (event, id: string) => contextFor(event.sender.id)?.controller.removeBookmark(id))
  ipcMain.handle('browser:create-bookmark-folder', (event, name: string, parentId?: string) => contextFor(event.sender.id)?.controller.createBookmarkFolder(name, parentId))
  ipcMain.handle('browser:remove-bookmark-folder', (event, id: string) => contextFor(event.sender.id)?.controller.removeBookmarkFolder(id))
  ipcMain.handle('browser:move-bookmark', (event, id: string, parentId?: string) => contextFor(event.sender.id)?.controller.moveBookmark(id, parentId))
  ipcMain.handle('browser:show-menu', (event, position: PopupPosition) => {
    const context = contextFor(event.sender.id)
    if (context) showBrowserMenu(context, position)
  })
  ipcMain.handle('browser:show-tab-menu', (event, request: TabMenuRequest) => {
    const context = contextFor(event.sender.id)
    if (context) showTabMenu(context, request)
  })
  ipcMain.handle('browser:show-bookmarks-bar-menu', (event, position: PopupPosition) => {
    const context = contextFor(event.sender.id)
    if (context) showBookmarksBarMenu(context, position)
  })
  ipcMain.handle('browser:show-bookmarks-overflow-menu', (event, request: BookmarksOverflowRequest) => {
    const context = contextFor(event.sender.id)
    if (context) showBookmarksOverflowMenu(context, request)
  })
  ipcMain.handle('browser:show-site-info', (event, position: PopupPosition) => {
    const context = contextFor(event.sender.id)
    if (context) showSiteInfo(context, position)
  })
  ipcMain.handle('browser:list-extensions', () => extensionService.list())
  ipcMain.handle('browser:install-extension', (event) => {
    const context = contextFor(event.sender.id)
    return context
      ? extensionService.install(context.window, context.controller.state().settings.language)
      : { ok: false, extensions: extensionService.list() }
  })
  ipcMain.handle('browser:remove-extension', (_event, id: string) => extensionService.remove(id))
  ipcMain.handle('browser:get-app-version', () => app.getVersion())
  ipcMain.handle('browser:check-for-updates', (_event, force?: boolean) => releaseService.check(force === true))
  ipcMain.handle('browser:open-latest-release', () => releaseService.openLatest())
  ipcMain.on('browser:set-content-bounds', (event, bounds: ContentBounds) => contextFor(event.sender.id)?.controller.setBounds(bounds))
}
