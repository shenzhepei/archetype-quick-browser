import { randomUUID } from 'node:crypto'
import { join } from 'node:path'
import { BaseWindow, WebContentsView, type WebContents } from 'electron'
import { normalizeAddress } from '@archetype/browser-runtime'
import type { BrowserState, BrowserTab, Language, RuntimeStatus, Theme } from '../shared.js'
import type { RuntimeClient } from './runtime-client.js'

interface TabRecord { state: BrowserTab; view: WebContentsView }

export class BrowserController {
  private readonly tabs = new Map<string, TabRecord>()
  private activeTabId = ''
  private bounds = { x: 0, y: 108, width: 1200, height: 700 }
  private language: Language = 'en'
  private theme: Theme = 'system'
  private runtime: RuntimeStatus = { configured: false }

  constructor(private readonly window: BaseWindow, private readonly shell: WebContents, private readonly pagePreload: string, private readonly runtimeClient: RuntimeClient) {}

  initialize(): void {
    this.createTab('archetype://runtime')
  }

  state(): BrowserState {
    return { tabs: [...this.tabs.values()].map((tab) => ({ ...tab.state })), activeTabId: this.activeTabId, language: this.language, theme: this.theme, runtime: this.runtime }
  }

  createTab(input = 'archetype://runtime'): void {
    const id = randomUUID()
    const url = normalizeAddress(input)
    const view = new WebContentsView({ webPreferences: { preload: this.pagePreload, sandbox: true, contextIsolation: true, nodeIntegration: false, partition: 'persist:archetype-runtime' } })
    const record: TabRecord = { state: { id, title: url.startsWith('archetype:') ? 'Runtime' : 'New tab', url, loading: false, canGoBack: false, canGoForward: false }, view }
    this.tabs.set(id, record)
    this.bind(record)
    this.selectTab(id)
    if (!url.startsWith('archetype:')) void view.webContents.loadURL(url)
  }

  closeTab(id: string): void {
    const record = this.tabs.get(id)
    if (!record) return
    const ids = [...this.tabs.keys()]
    const index = ids.indexOf(id)
    this.window.contentView.removeChildView(record.view)
    record.view.webContents.close()
    this.tabs.delete(id)
    if (this.tabs.size === 0) return this.createTab()
    if (id === this.activeTabId) this.selectTab(ids[index + 1] ?? ids[index - 1]!)
    this.publish()
  }

  selectTab(id: string): void {
    const next = this.tabs.get(id)
    if (!next) return
    const current = this.tabs.get(this.activeTabId)
    if (current) this.window.contentView.removeChildView(current.view)
    this.activeTabId = id
    if (!next.state.url.startsWith('archetype:')) {
      this.window.contentView.addChildView(next.view)
      next.view.setBounds(this.bounds)
    }
    this.publish()
    void this.refreshRuntime()
  }

  navigate(input: string): void {
    const tab = this.active()
    const url = normalizeAddress(input)
    tab.state.url = url
    if (url.startsWith('archetype:')) {
      this.window.contentView.removeChildView(tab.view)
      tab.state.title = 'Runtime'
      tab.state.loading = false
      this.publish()
      void this.refreshRuntime()
      return
    }
    if (!this.window.contentView.children.includes(tab.view)) this.window.contentView.addChildView(tab.view)
    tab.view.setBounds(this.bounds)
    void tab.view.webContents.loadURL(url)
  }

  setBounds(bounds: { x: number; y: number; width: number; height: number }): void {
    this.bounds = Object.fromEntries(Object.entries(bounds).map(([key, value]) => [key, Math.max(0, Math.round(value))])) as typeof bounds
    const tab = this.tabs.get(this.activeTabId)
    if (tab && !tab.state.url.startsWith('archetype:')) tab.view.setBounds(this.bounds)
  }

  back(): void { if (this.active().view.webContents.navigationHistory.canGoBack()) this.active().view.webContents.navigationHistory.goBack() }
  forward(): void { if (this.active().view.webContents.navigationHistory.canGoForward()) this.active().view.webContents.navigationHistory.goForward() }
  reload(): void { if (!this.active().state.url.startsWith('archetype:')) this.active().view.webContents.reload() }

  updatePreferences(value: { language?: Language; theme?: Theme }): void {
    if (value.language) this.language = value.language
    if (value.theme) this.theme = value.theme
    this.publish()
  }

  private active(): TabRecord {
    const tab = this.tabs.get(this.activeTabId)
    if (!tab) throw new Error('No active browser tab.')
    return tab
  }

  private bind(record: TabRecord): void {
    const contents = record.view.webContents
    contents.setWindowOpenHandler(({ url }) => { this.createTab(url); return { action: 'deny' } })
    contents.on('did-start-loading', () => { record.state.loading = true; this.publish() })
    contents.on('did-stop-loading', () => { record.state.loading = false; this.sync(record); this.publish(); if (record.state.id === this.activeTabId) void this.refreshRuntime() })
    contents.on('did-navigate', (_event, url) => { record.state.url = url; this.sync(record); this.publish() })
    contents.on('page-title-updated', (event, title) => { event.preventDefault(); record.state.title = title; this.publish() })
  }

  private sync(record: TabRecord): void {
    record.state.url = record.view.webContents.getURL() || record.state.url
    record.state.canGoBack = record.view.webContents.navigationHistory.canGoBack()
    record.state.canGoForward = record.view.webContents.navigationHistory.canGoForward()
  }

  private async refreshRuntime(): Promise<void> {
    const tab = this.tabs.get(this.activeTabId)
    if (!tab || tab.state.url.startsWith('archetype:')) return
    try {
      const project = await this.runtimeClient.discover(tab.state.url)
      const session = await this.runtimeClient.session(tab.state.url).catch(() => null)
      this.runtime = { configured: true, project, session }
    } catch (error) {
      this.runtime = { configured: false, error: error instanceof Error ? error.message : 'Runtime discovery failed.' }
    }
    this.publish()
  }

  private publish(): void {
    if (!this.shell.isDestroyed()) this.shell.send('browser:state', this.state())
  }
}
