export type ThemePreference = 'system' | 'light' | 'dark'
export type Language = 'en' | 'zh-CN'

export interface TabState {
  id: string
  url: string
  title: string
  favicon?: string
  loading: boolean
  canGoBack: boolean
  canGoForward: boolean
}

export interface Bookmark {
  id: string
  title: string
  url: string
  createdAt: number
}

export interface HistoryEntry {
  id: string
  title: string
  url: string
  visitedAt: number
}

export interface BrowserSettings {
  theme: ThemePreference
  language: Language
}

export interface BrowserState {
  tabs: TabState[]
  activeTabId: string
  bookmarks: Bookmark[]
  history: HistoryEntry[]
  settings: BrowserSettings
}

export interface ContentBounds {
  x: number
  y: number
  width: number
  height: number
}

export interface ArchetypeBridge {
  getState(): Promise<BrowserState>
  newTab(url?: string): Promise<void>
  selectTab(id: string): Promise<void>
  closeTab(id: string): Promise<void>
  navigate(input: string): Promise<void>
  back(): Promise<void>
  forward(): Promise<void>
  reload(): Promise<void>
  stop(): Promise<void>
  toggleBookmark(): Promise<void>
  openInternal(path: 'history' | 'settings/appearance' | 'settings/about'): Promise<void>
  updateSettings(settings: Partial<BrowserSettings>): Promise<void>
  clearHistory(): Promise<void>
  setContentBounds(bounds: ContentBounds): void
  onState(callback: (state: BrowserState) => void): () => void
}
