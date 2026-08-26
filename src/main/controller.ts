import { randomUUID } from 'node:crypto'
import { join } from 'node:path'
import { app, BaseWindow, dialog, Menu, WebContents, WebContentsView } from 'electron'
import type { ContextMenuParams } from 'electron'
import { internalPageTitle, isRecordableHistoryEntry } from '../shared/browser'
import type {
  Bookmark,
  BookmarkFolder,
  BrowserSettings,
  BrowserState,
  ContentBounds,
  HistoryEntry,
  TabState
} from '../shared/browser'
import { BrowserStore } from './store'
import { buildPageContextMenu, pageMenuLabels, pageSaveFilename } from './page-context-menu'
import { SiteSecurityService } from './site-security'

interface BrowserTab {
  state: TabState
  view: WebContentsView
}

const INTERNAL_PREFIX = 'archetype://'

export function normalizeAddress(input: string): string {
  const value = input.trim()
  if (!value) return 'about:blank'
  if (/^(https?|file|about|archetype|view-source):/i.test(value)) return value
  if (/^(localhost|\d{1,3}(\.\d{1,3}){3})(:\d+)?(\/|$)/i.test(value)) return `http://${value}`
  if (/^[\w-]+(\.[\w-]+)+(\/.*)?$/i.test(value)) return `https://${value}`
  return `https://www.google.com/search?q=${encodeURIComponent(value)}`
}

export class BrowserController {
  private readonly tabs = new Map<string, BrowserTab>()
  private activeTabId = ''
  private bounds: ContentBounds = { x: 0, y: 132, width: 1000, height: 600 }
  private bookmarks: Bookmark[] = []
  private bookmarkFolders: BookmarkFolder[] = []
  private history: HistoryEntry[] = []
  private settings: BrowserSettings = { theme: 'system', language: 'en' }

  constructor(
    private readonly window: BaseWindow,
    private readonly shellContents: WebContents,
    private readonly store: BrowserStore,
    private readonly siteSecurity: SiteSecurityService,
    private readonly persistTabs = true
  ) {}

  async initialize(): Promise<void> {
    const saved = this.store.snapshot()
    this.bookmarks = saved.bookmarks
    this.bookmarkFolders = saved.bookmarkFolders
    this.history = saved.history.filter((entry) => isRecordableHistoryEntry(entry.title, entry.url))
    this.settings = saved.settings
    if (this.persistTabs) {
      for (const tab of saved.tabs) this.createTab(tab.url, tab.title, false)
    } else {
      this.createTab('about:blank', 'New tab', false)
    }
    const selected = [...this.tabs.keys()][this.persistTabs ? Math.min(saved.activeTab, this.tabs.size - 1) : 0]
    this.selectTab(selected, false)
  }

  state(): BrowserState {
    return {
      tabs: [...this.tabs.values()].map(({ state }) => ({ ...state })),
      activeTabId: this.activeTabId,
      bookmarks: structuredClone(this.bookmarks),
      bookmarkFolders: structuredClone(this.bookmarkFolders),
      history: structuredClone(this.history),
      settings: { ...this.settings },
      siteInfo: this.siteSecurity.infoFor(this.tabs.get(this.activeTabId)?.state.url ?? '')
    }
  }

  createTab(input = 'about:blank', title = 'New tab', select = true): void {
    const id = randomUUID()
    const url = normalizeAddress(input)
    const resolvedTitle = url.startsWith(INTERNAL_PREFIX) ? internalPageTitle(url, this.settings.language) : title
    const view = new WebContentsView({
      webPreferences: {
        contextIsolation: true,
        sandbox: true,
        nodeIntegration: false,
        partition: 'persist:archetype'
      }
    })
    const tab: BrowserTab = {
      view,
      state: { id, url, title: resolvedTitle, loading: false, canGoBack: false, canGoForward: false }
    }
    this.tabs.set(id, tab)
    this.bindTab(tab)
    if (!url.startsWith(INTERNAL_PREFIX)) void view.webContents.loadURL(url)
    if (select) this.selectTab(id)
  }

  selectTab(id: string, persist = true): void {
    const next = this.tabs.get(id)
    if (!next) return
    const current = this.tabs.get(this.activeTabId)
    if (current) this.window.contentView.removeChildView(current.view)
    this.activeTabId = id
    if (!next.state.url.startsWith(INTERNAL_PREFIX)) {
      this.window.contentView.addChildView(next.view)
      next.view.setBounds(this.bounds)
    }
    this.publish()
    if (persist) void this.persist()
  }

  closeTab(id: string): void {
    const ids = [...this.tabs.keys()]
    const index = ids.indexOf(id)
    if (index < 0) return
    this.closeTabs([id], ids[index + 1] ?? ids[index - 1])
  }

  reloadTab(id: string): void {
    const tab = this.tabs.get(id)
    if (!tab) return
    if (tab.state.url.startsWith(INTERNAL_PREFIX)) {
      tab.state.title = internalPageTitle(tab.state.url, this.settings.language)
      this.publish()
    } else {
      tab.view.webContents.reload()
    }
  }

  closeOtherTabs(id: string): void {
    if (!this.tabs.has(id)) return
    this.closeTabs([...this.tabs.keys()].filter((tabId) => tabId !== id), id)
  }

  closeTabsToRight(id: string): void {
    const ids = [...this.tabs.keys()]
    const index = ids.indexOf(id)
    if (index < 0) return
    this.closeTabs(ids.slice(index + 1), id)
  }

  navigate(input: string): void {
    const tab = this.activeTab()
    const url = normalizeAddress(input)
    tab.state.url = url
    tab.state.favicon = undefined
    if (url.startsWith(INTERNAL_PREFIX)) {
      this.window.contentView.removeChildView(tab.view)
      tab.state.title = internalPageTitle(url, this.settings.language)
      tab.state.loading = false
      this.publish()
      void this.persist()
      return
    }
    if (!this.window.contentView.children.includes(tab.view)) {
      this.window.contentView.addChildView(tab.view)
      tab.view.setBounds(this.bounds)
    }
    void tab.view.webContents.loadURL(url)
    this.publish()
  }

  openUtilityPage(path: 'history' | 'bookmarks' | 'extensions' | 'settings/appearance'): void {
    if (path === 'bookmarks') {
      this.openBookmarkManager()
      return
    }
    const url = `${INTERNAL_PREFIX}${path}`
    const existing = [...this.tabs.entries()].find(([, tab]) =>
      path.startsWith('settings/')
        ? tab.state.url.startsWith(`${INTERNAL_PREFIX}settings/`)
        : tab.state.url === url
    )
    if (existing) {
      this.selectTab(existing[0])
      return
    }
    this.createTab(url, internalPageTitle(url, this.settings.language))
  }

  openBookmarkManager(createFolder = false): void {
    const url = `${INTERNAL_PREFIX}bookmarks${createFolder ? '/new-folder' : ''}`
    const existing = [...this.tabs.entries()].find(([, tab]) => tab.state.url.startsWith(`${INTERNAL_PREFIX}bookmarks`))
    if (existing) {
      this.selectTab(existing[0])
      if (createFolder) this.navigate(url)
      return
    }
    this.createTab(url, internalPageTitle(url, this.settings.language))
  }

  back(): void {
    const contents = this.activeTab().view.webContents
    if (contents.navigationHistory.canGoBack()) contents.navigationHistory.goBack()
  }

  forward(): void {
    const contents = this.activeTab().view.webContents
    if (contents.navigationHistory.canGoForward()) contents.navigationHistory.goForward()
  }

  reload(): void {
    this.activeTab().view.webContents.reload()
  }

  stop(): void {
    this.activeTab().view.webContents.stop()
  }

  canPrint(): boolean {
    return this.canPrintTab(this.activeTab())
  }

  print(): void {
    this.printTab(this.activeTab())
  }

  setBounds(bounds: ContentBounds): void {
    this.bounds = {
      x: Math.max(0, Math.round(bounds.x)),
      y: Math.max(0, Math.round(bounds.y)),
      width: Math.max(0, Math.round(bounds.width)),
      height: Math.max(0, Math.round(bounds.height))
    }
    const tab = this.tabs.get(this.activeTabId)
    if (tab && !tab.state.url.startsWith(INTERNAL_PREFIX)) tab.view.setBounds(this.bounds)
  }

  toggleBookmark(): void {
    const tab = this.activeTab()
    const index = this.bookmarks.findIndex((bookmark) => bookmark.url === tab.state.url)
    if (index < 0) {
      this.addActivePageBookmark()
      return
    }
    this.bookmarks.splice(index, 1)
    this.publish()
    void this.persist()
  }

  canAddActivePageBookmark(): boolean {
    const tab = this.activeTab()
    return /^(https?|file):/i.test(tab.state.url) && !this.bookmarks.some((bookmark) => bookmark.url === tab.state.url)
  }

  addActivePageBookmark(): void {
    if (!this.canAddActivePageBookmark()) return
    const tab = this.activeTab()
    const bookmark = {
      id: randomUUID(),
      title: tab.state.title,
      url: tab.state.url,
      favicon: tab.state.favicon,
      createdAt: Date.now()
    }
    this.bookmarks.push(bookmark)
    if (bookmark.favicon) void this.cacheBookmarkFavicon(bookmark.id, bookmark.favicon, tab.view.webContents)
    this.publish()
    void this.persist()
  }

  updateSettings(settings: Partial<BrowserSettings>): void {
    this.settings = { ...this.settings, ...settings }
    if (settings.language) {
      for (const tab of this.tabs.values()) {
        if (tab.state.url.startsWith(INTERNAL_PREFIX)) {
          tab.state.title = internalPageTitle(tab.state.url, settings.language)
        }
      }
    }
    this.publish()
    void this.persist()
  }

  clearHistory(): void {
    this.history = []
    this.publish()
    void this.persist()
  }

  removeBookmark(id: string): void {
    const index = this.bookmarks.findIndex((bookmark) => bookmark.id === id)
    if (index < 0) return
    this.bookmarks.splice(index, 1)
    this.publish()
    void this.persist()
  }

  createBookmarkFolder(name: string, parentId?: string): void {
    const normalizedName = name.trim().slice(0, 80)
    if (!normalizedName || (parentId && !this.bookmarkFolders.some((folder) => folder.id === parentId))) return
    this.bookmarkFolders.push({ id: randomUUID(), name: normalizedName, parentId, createdAt: Date.now() })
    this.publish()
    void this.persist()
  }

  removeBookmarkFolder(id: string): void {
    if (!this.bookmarkFolders.some((folder) => folder.id === id)) return
    const removed = new Set([id])
    let changed = true
    while (changed) {
      changed = false
      for (const folder of this.bookmarkFolders) {
        if (folder.parentId && removed.has(folder.parentId) && !removed.has(folder.id)) {
          removed.add(folder.id)
          changed = true
        }
      }
    }
    this.bookmarkFolders = this.bookmarkFolders.filter((folder) => !removed.has(folder.id))
    this.bookmarks = this.bookmarks.filter((bookmark) => !bookmark.parentId || !removed.has(bookmark.parentId))
    this.publish()
    void this.persist()
  }

  moveBookmark(id: string, parentId?: string): void {
    const bookmark = this.bookmarks.find((entry) => entry.id === id)
    if (!bookmark || (parentId && !this.bookmarkFolders.some((folder) => folder.id === parentId))) return
    bookmark.parentId = parentId
    this.publish()
    void this.persist()
  }

  refreshSiteInfo(): void {
    this.publish()
  }

  dispose(): void {
    for (const tab of this.tabs.values()) tab.view.webContents.close()
    this.tabs.clear()
  }

  private activeTab(): BrowserTab {
    const tab = this.tabs.get(this.activeTabId)
    if (!tab) throw new Error('Active browser tab is unavailable')
    return tab
  }

  private closeTabs(ids: string[], fallbackId?: string): void {
    const closing = new Set(ids.filter((id) => this.tabs.has(id)))
    if (closing.size === 0) return
    for (const id of closing) {
      const tab = this.tabs.get(id)!
      if (this.window.contentView.children.includes(tab.view)) this.window.contentView.removeChildView(tab.view)
      tab.view.webContents.close()
      this.tabs.delete(id)
    }
    if (this.tabs.size === 0) {
      this.createTab()
    } else if (closing.has(this.activeTabId)) {
      this.selectTab(fallbackId && this.tabs.has(fallbackId) ? fallbackId : this.tabs.keys().next().value!)
    } else {
      this.publish()
      void this.persist()
    }
  }

  private bindTab(tab: BrowserTab): void {
    const contents = tab.view.webContents
    contents.setWindowOpenHandler(({ url }) => {
      this.createTab(url)
      return { action: 'deny' }
    })
    contents.on('context-menu', (_event, params) => this.showPageContextMenu(tab, params))
    contents.on('did-start-loading', () => this.updateTab(tab, { loading: true }))
    contents.on('did-stop-loading', () => this.updateNavigationState(tab, false))
    contents.on('did-navigate', (_event, url) => {
      this.updateNavigationState(tab, false, url)
      this.recordHistory(tab)
    })
    contents.on('did-navigate-in-page', (_event, url, isMainFrame) => {
      if (isMainFrame) this.updateNavigationState(tab, false, url)
    })
    contents.on('page-title-updated', (event, title) => {
      event.preventDefault()
      this.updateTab(tab, { title: title || tab.state.url })
      this.recordHistory(tab)
    })
    contents.on('page-favicon-updated', (_event, favicons) => {
      const favicon = favicons[0]
      this.updateTab(tab, { favicon })
      if (favicon) this.updateBookmarkFavicon(tab, favicon)
    })
    contents.on('render-process-gone', () => this.updateTab(tab, { loading: false }))
  }

  private showPageContextMenu(tab: BrowserTab, params: ContextMenuParams): void {
    const contents = tab.view.webContents
    const url = contents.getURL() || tab.state.url
    const navigation = contents.navigationHistory
    const isPageUrl = /^(https?|file):/i.test(url)
    const canPrint = this.canPrintTab(tab)
    const canViewSource = /^https?:/i.test(url)
    const isAlive = (): boolean => !contents.isDestroyed()
    const template = buildPageContextMenu(this.settings.language, {
      canGoBack: navigation.canGoBack(),
      canGoForward: navigation.canGoForward(),
      canSave: isPageUrl,
      canPrint,
      canViewSource
    }, {
      back: () => {
        if (isAlive() && navigation.canGoBack()) navigation.goBack()
      },
      forward: () => {
        if (isAlive() && navigation.canGoForward()) navigation.goForward()
      },
      reload: () => {
        if (isAlive()) contents.reload()
      },
      savePage: () => {
        if (isAlive() && isPageUrl) void this.savePage(tab)
      },
      printPage: () => {
        if (isAlive() && canPrint) this.printTab(tab)
      },
      viewSource: () => {
        if (isAlive() && canViewSource) this.createTab(`view-source:${url}`, `Source: ${tab.state.title}`)
      },
      inspect: () => {
        if (isAlive()) contents.inspectElement(params.x, params.y)
      }
    })
    Menu.buildFromTemplate(template).popup({ window: this.window })
  }

  private canPrintTab(tab: BrowserTab): boolean {
    return /^(https?|file|view-source):/i.test(tab.view.webContents.getURL() || tab.state.url)
  }

  private printTab(tab: BrowserTab): void {
    const contents = tab.view.webContents
    if (contents.isDestroyed() || !this.canPrintTab(tab)) return
    const labels = pageMenuLabels[this.settings.language]
    contents.print({ printBackground: true }, (success, failureReason) => {
      if (success || contents.isDestroyed() || /cancel/i.test(failureReason)) return
      void dialog.showMessageBox(this.window, {
        type: 'error',
        title: labels.printFailed,
        message: labels.printFailed,
        detail: /printer/i.test(failureReason) ? labels.noPrinters : labels.printFailedDetail
      })
    })
  }

  private async savePage(tab: BrowserTab): Promise<void> {
    const contents = tab.view.webContents
    if (contents.isDestroyed()) return
    const labels = pageMenuLabels[this.settings.language]
    const result = await dialog.showSaveDialog(this.window, {
      title: labels.savePageAs,
      defaultPath: join(app.getPath('downloads'), pageSaveFilename(tab.state.title, contents.getURL() || tab.state.url)),
      filters: [{ name: labels.htmlFile, extensions: ['html'] }]
    })
    if (result.canceled || !result.filePath || contents.isDestroyed()) return
    try {
      await contents.savePage(result.filePath, 'HTMLComplete')
    } catch (error) {
      await dialog.showMessageBox(this.window, {
        type: 'error',
        title: labels.saveFailed,
        message: labels.saveFailed,
        detail: error instanceof Error ? error.message : labels.saveFailedDetail
      })
    }
  }

  private updateNavigationState(tab: BrowserTab, loading: boolean, url?: string): void {
    const history = tab.view.webContents.navigationHistory
    this.updateTab(tab, {
      loading,
      url: url ?? (tab.view.webContents.getURL() || tab.state.url),
      canGoBack: history.canGoBack(),
      canGoForward: history.canGoForward()
    })
    void this.persist()
  }

  private async cacheBookmarkFavicon(bookmarkId: string, url: string, contents: WebContents): Promise<void> {
    try {
      const response = await contents.session.fetch(url)
      if (!response.ok) return
      const mimeType = response.headers.get('content-type')?.split(';')[0]?.trim()
      const declaredSize = Number(response.headers.get('content-length') ?? 0)
      if (!mimeType?.startsWith('image/') || declaredSize > 1024 * 1024) return
      const bytes = Buffer.from(await response.arrayBuffer())
      if (bytes.byteLength === 0 || bytes.byteLength > 1024 * 1024) return
      const bookmark = this.bookmarks.find((entry) => entry.id === bookmarkId)
      if (!bookmark) return
      bookmark.favicon = `data:${mimeType};base64,${bytes.toString('base64')}`
      this.publish()
      await this.persist()
    } catch {
      // The original favicon URL remains usable when caching is unavailable.
    }
  }

  private updateBookmarkFavicon(tab: BrowserTab, favicon: string): void {
    const bookmark = this.bookmarks.find((entry) => entry.url === tab.state.url)
    if (!bookmark || bookmark.favicon === favicon || bookmark.favicon?.startsWith('data:image/')) return
    bookmark.favicon = favicon
    this.publish()
    void this.persist()
    void this.cacheBookmarkFavicon(bookmark.id, favicon, tab.view.webContents)
  }

  private updateTab(tab: BrowserTab, update: Partial<TabState>): void {
    Object.assign(tab.state, update)
    this.publish()
  }

  private recordHistory(tab: BrowserTab): void {
    const { url, title } = tab.state
    if (!isRecordableHistoryEntry(title, url)) return
    const latest = this.history[0]
    if (latest?.url === url && Date.now() - latest.visitedAt <= 5000) {
      latest.title = title
      latest.visitedAt = Date.now()
      this.publish()
      void this.persist()
      return
    }
    this.history.unshift({ id: randomUUID(), url, title, visitedAt: Date.now() })
    this.history = this.history.slice(0, 1000)
    this.publish()
    void this.persist()
  }

  private publish(): void {
    if (!this.shellContents.isDestroyed()) this.shellContents.send('browser:state', this.state())
  }

  private async persist(): Promise<void> {
    const tabs = [...this.tabs.values()].map(({ state }) => ({ url: state.url, title: state.title }))
    const activeTab = Math.max(0, [...this.tabs.keys()].indexOf(this.activeTabId))
    await this.store.update((state) => {
      if (this.persistTabs) {
        state.tabs = tabs
        state.activeTab = activeTab
      }
      state.bookmarks = this.bookmarks
      state.bookmarkFolders = this.bookmarkFolders
      state.history = this.history
      state.settings = this.settings
    })
  }
}
