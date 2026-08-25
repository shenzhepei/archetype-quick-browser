import type {
  ArchetypeBridge,
  BrowserSettings,
  BrowserState,
  ContentBounds,
  TabState
} from '../../shared/browser'

let state: BrowserState = {
  tabs: [
    {
      id: 'preview',
      url: 'https://example.com',
      title: 'Example Domain',
      loading: false,
      canGoBack: false,
      canGoForward: false
    }
  ],
  activeTabId: 'preview',
  bookmarks: [
    { id: 'docs', title: 'Archetype', url: 'https://github.com/shenzhepei/archetype-quick-browser', createdAt: 0 }
  ],
  history: [],
  settings: { theme: 'system', language: 'en' },
  siteInfo: { url: 'https://example.com', origin: 'https://example.com', connection: 'secure', permissions: [] }
}

const listeners = new Set<(next: BrowserState) => void>()
const publish = (): void => listeners.forEach((listener) => listener(structuredClone(state)))
const active = (): TabState => state.tabs.find((tab) => tab.id === state.activeTabId)!

export const demoBridge: ArchetypeBridge = {
  platform: 'web',
  getState: async () => structuredClone(state),
  newTab: async (url = 'about:blank') => {
    const tab = { id: crypto.randomUUID(), url, title: 'New tab', loading: false, canGoBack: false, canGoForward: false }
    state = { ...state, tabs: [...state.tabs, tab], activeTabId: tab.id }
    publish()
  },
  selectTab: async (id) => {
    state = { ...state, activeTabId: id }
    publish()
  },
  closeTab: async (id) => {
    const remaining = state.tabs.filter((tab) => tab.id !== id)
    state = { ...state, tabs: remaining, activeTabId: remaining[0]?.id ?? '' }
    publish()
  },
  navigate: async (url) => {
    Object.assign(active(), { url, title: url || 'New tab' })
    publish()
  },
  back: async () => undefined,
  forward: async () => undefined,
  reload: async () => undefined,
  stop: async () => undefined,
  toggleBookmark: async () => {
    const tab = active()
    const exists = state.bookmarks.some((bookmark) => bookmark.url === tab.url)
    state = {
      ...state,
      bookmarks: exists
        ? state.bookmarks.filter((bookmark) => bookmark.url !== tab.url)
        : [...state.bookmarks, { id: crypto.randomUUID(), title: tab.title, url: tab.url, createdAt: Date.now() }]
    }
    publish()
  },
  openInternal: async (path) => {
    Object.assign(active(), { url: `archetype://${path}`, title: path.startsWith('history') ? 'History' : 'Settings' })
    publish()
  },
  openUtility: async (path) => {
    const prefix = path.startsWith('settings/') ? 'archetype://settings/' : `archetype://${path}`
    const existing = state.tabs.find((tab) => tab.url.startsWith(prefix))
    if (existing) {
      state = { ...state, activeTabId: existing.id }
    } else {
      const tab = { id: crypto.randomUUID(), url: `archetype://${path}`, title: path === 'history' ? 'History' : 'Settings', loading: false, canGoBack: false, canGoForward: false }
      state = { ...state, tabs: [...state.tabs, tab], activeTabId: tab.id }
    }
    publish()
  },
  updateSettings: async (settings: Partial<BrowserSettings>) => {
    state = { ...state, settings: { ...state.settings, ...settings } }
    publish()
  },
  clearHistory: async () => {
    state = { ...state, history: [] }
    publish()
  },
  showMenu: async () => undefined,
  showTabMenu: async () => undefined,
  showSiteInfo: async () => undefined,
  getAppVersion: async () => '0.1.0',
  checkForUpdates: async () => ({ currentVersion: '0.1.0', checkedAt: Date.now(), state: 'unavailable' }),
  openLatestRelease: async () => undefined,
  setContentBounds: (_bounds: ContentBounds) => undefined,
  onState: (callback) => {
    listeners.add(callback)
    return () => listeners.delete(callback)
  }
}
