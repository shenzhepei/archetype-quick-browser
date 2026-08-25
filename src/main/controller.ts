import { randomUUID } from 'node:crypto'
import { BaseWindow, WebContents, WebContentsView, session } from 'electron'
import type {
  Bookmark,
  BrowserSettings,
  BrowserState,
  ContentBounds,
  HistoryEntry,
  TabState
} from '../shared/browser'
import { BrowserStore } from './store'

interface BrowserTab {
  state: TabState
  view: WebContentsView
}

const INTERNAL_PREFIX = 'archetype://'

export function normalizeAddress(input: string): string {
  const value = input.trim()
  if (!value) return 'about:blank'
  if (/^(https?|file|about|archetype):/i.test(value)) return value
  if (/^(localhost|\d{1,3}(\.\d{1,3}){3})(:\d+)?(\/|$)/i.test(value)) return `http://${value}`
  if (/^[\w-]+(\.[\w-]+)+(\/.*)?$/i.test(value)) return `https://${value}`
  return `https://www.google.com/search?q=${encodeURIComponent(value)}`
}

export class BrowserController {
  private readonly tabs = new Map<string, BrowserTab>()
  private activeTabId = ''
  private bounds: ContentBounds = { x: 0, y: 132, width: 1000, height: 600 }
  private bookmarks: Bookmark[] = []
  private history: HistoryEntry[] = []
  private settings: BrowserSettings = { theme: 'system', language: 'en' }

  constructor(
    private readonly window: BaseWindow,
    private readonly shellContents: WebContents,
    private readonly store: BrowserStore
  ) {}

  async initialize(): Promise<void> {
    await this.store.load()
    const saved = this.store.snapshot()
    this.bookmarks = saved.bookmarks
    this.history = saved.history
    this.settings = saved.settings
    for (const tab of saved.tabs) this.createTab(tab.url, tab.title, false)
    const selected = [...this.tabs.keys()][Math.min(saved.activeTab, this.tabs.size - 1)]
    this.selectTab(selected, false)
  }

  state(): BrowserState {
    return {
      tabs: [...this.tabs.values()].map(({ state }) => ({ ...state })),
      activeTabId: this.activeTabId,
      bookmarks: structuredClone(this.bookmarks),
      history: structuredClone(this.history),
      settings: { ...this.settings }
    }
  }

  createTab(input = 'about:blank', title = 'New tab', select = true): void {
    const id = randomUUID()
    const url = normalizeAddress(input)
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
      state: { id, url, title, loading: false, canGoBack: false, canGoForward: false }
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
    const tab = this.tabs.get(id)
    if (!tab) return
    this.window.contentView.removeChildView(tab.view)
    tab.view.webContents.close()
    this.tabs.delete(id)
    if (this.tabs.size === 0) {
      this.createTab()
      return
    }
    if (id === this.activeTabId) this.selectTab(ids[index + 1] ?? ids[index - 1])
    else {
      this.publish()
      void this.persist()
    }
  }

  navigate(input: string): void {
    const tab = this.activeTab()
    const url = normalizeAddress(input)
    tab.state.url = url
    tab.state.favicon = undefined
    if (url.startsWith(INTERNAL_PREFIX)) {
      this.window.contentView.removeChildView(tab.view)
      tab.state.title = this.internalTitle(url)
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
    const tab = this.activeTab().state
    const index = this.bookmarks.findIndex((bookmark) => bookmark.url === tab.url)
    if (index >= 0) this.bookmarks.splice(index, 1)
    else if (!tab.url.startsWith(INTERNAL_PREFIX) && tab.url !== 'about:blank') {
      this.bookmarks.push({ id: randomUUID(), title: tab.title, url: tab.url, createdAt: Date.now() })
    }
    this.publish()
    void this.persist()
  }

  updateSettings(settings: Partial<BrowserSettings>): void {
    this.settings = { ...this.settings, ...settings }
    this.publish()
    void this.persist()
  }

  clearHistory(): void {
    this.history = []
    this.publish()
    void this.persist()
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

  private bindTab(tab: BrowserTab): void {
    const contents = tab.view.webContents
    contents.setWindowOpenHandler(({ url }) => {
      this.createTab(url)
      return { action: 'deny' }
    })
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
    })
    contents.on('page-favicon-updated', (_event, favicons) => {
      this.updateTab(tab, { favicon: favicons[0] })
    })
    contents.on('render-process-gone', () => this.updateTab(tab, { loading: false }))
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

  private updateTab(tab: BrowserTab, update: Partial<TabState>): void {
    Object.assign(tab.state, update)
    this.publish()
  }

  private recordHistory(tab: BrowserTab): void {
    const { url, title } = tab.state
    if (!/^https?:/i.test(url)) return
    this.history.unshift({ id: randomUUID(), url, title, visitedAt: Date.now() })
    this.history = this.history.slice(0, 1000)
    this.publish()
    void this.persist()
  }

  private internalTitle(url: string): string {
    if (url.includes('history')) return 'History'
    if (url.includes('about')) return 'About Archetype'
    return 'Settings'
  }

  private publish(): void {
    if (!this.shellContents.isDestroyed()) this.shellContents.send('browser:state', this.state())
  }

  private async persist(): Promise<void> {
    const tabs = [...this.tabs.values()].map(({ state }) => ({ url: state.url, title: state.title }))
    const activeTab = Math.max(0, [...this.tabs.keys()].indexOf(this.activeTabId))
    await this.store.update((state) => {
      state.tabs = tabs
      state.activeTab = activeTab
      state.bookmarks = this.bookmarks
      state.history = this.history
      state.settings = this.settings
    })
  }
}

export function configureSession(): void {
  const browserSession = session.fromPartition('persist:archetype')
  browserSession.setPermissionRequestHandler((_webContents, _permission, callback) => callback(false))
}
