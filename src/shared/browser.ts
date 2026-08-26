export type ThemePreference = 'system' | 'light' | 'dark'
export type Language = 'en' | 'zh-CN'

export function isRecordableHistoryEntry(title: string, url: string): boolean {
  const normalizedTitle = title.trim().toLocaleLowerCase()
  return /^https?:/i.test(url)
    && normalizedTitle.length > 0
    && normalizedTitle !== 'new tab'
    && normalizedTitle !== '新标签页'
}

export function internalPageTitle(url: string, language: Language): string {
  const chinese = language === 'zh-CN'
  if (url.includes('history')) return chinese ? '历史记录' : 'History'
  if (url.includes('bookmarks')) return chinese ? '书签' : 'Bookmarks'
  if (url.includes('extensions')) return chinese ? '扩展程序' : 'Extensions'
  if (url.includes('/languages')) return chinese ? '设置 - 语言' : 'Settings - Language'
  if (url.includes('/about')) return chinese ? '设置 - 关于 Archetype' : 'Settings - About Archetype'
  return chinese ? '设置 - 外观' : 'Settings - Appearance'
}

export type ReleaseCheckState = 'up-to-date' | 'update-available' | 'no-release' | 'unavailable'

export interface ReleaseStatus {
  currentVersion: string
  latestVersion?: string
  releaseUrl?: string
  checkedAt: number
  state: ReleaseCheckState
}

export type SiteConnection = 'secure' | 'verifying' | 'insecure' | 'local' | 'internal' | 'none'

export interface CertificateSummary {
  subjectName: string
  issuerName: string
  validStart: number
  validExpiry: number
  fingerprint: string
  serialNumber: string
  isIssuedByKnownRoot: boolean
  verificationResult: string
  errorCode: number
}

export interface SitePermissionRecord {
  permission: string
  state: 'granted' | 'blocked'
}

export interface SiteInfo {
  url: string
  origin?: string
  connection: SiteConnection
  certificate?: CertificateSummary
  permissions: SitePermissionRecord[]
}

export function baseSiteInfo(url: string): SiteInfo {
  if (!url || url === 'about:blank') return { url, connection: 'none', permissions: [] }
  if (url.startsWith('archetype://')) return { url, connection: 'internal', permissions: [] }
  if (url.startsWith('file:')) return { url, connection: 'local', permissions: [] }
  try {
    const parsed = new URL(url)
    if (parsed.protocol === 'https:') return { url, origin: parsed.origin, connection: 'verifying', permissions: [] }
    if (parsed.protocol === 'http:') return { url, origin: parsed.origin, connection: 'insecure', permissions: [] }
  } catch {
    return { url, connection: 'none', permissions: [] }
  }
  return { url, connection: 'none', permissions: [] }
}

export function isReleaseNewer(latest: string, current: string): boolean | undefined {
  const parse = (value: string): { parts: number[]; prerelease: boolean } | undefined => {
    const match = value.trim().match(/^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/)
    if (!match) return undefined
    return { parts: [Number(match[1]), Number(match[2]), Number(match[3])], prerelease: Boolean(match[4]) }
  }
  const next = parse(latest)
  const installed = parse(current)
  if (!next || !installed) return undefined
  for (let index = 0; index < next.parts.length; index += 1) {
    if (next.parts[index] !== installed.parts[index]) return next.parts[index] > installed.parts[index]
  }
  return installed.prerelease && !next.prerelease
}

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
  favicon?: string
  parentId?: string
  createdAt: number
}

export interface BookmarkFolder {
  id: string
  name: string
  parentId?: string
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

export interface BrowserExtension {
  id: string
  name: string
  version: string
  description?: string
  path: string
}

export interface ExtensionOperationResult {
  ok: boolean
  canceled?: boolean
  extensions: BrowserExtension[]
}

export interface BrowserState {
  tabs: TabState[]
  activeTabId: string
  bookmarks: Bookmark[]
  bookmarkFolders: BookmarkFolder[]
  history: HistoryEntry[]
  settings: BrowserSettings
  siteInfo: SiteInfo
}

export interface ContentBounds {
  x: number
  y: number
  width: number
  height: number
}

export interface PopupPosition {
  x: number
  y: number
}

export interface TabMenuRequest extends PopupPosition {
  tabId: string
}

export interface BookmarksOverflowRequest extends PopupPosition {
  bookmarkIds: string[]
}

export interface ArchetypeBridge {
  platform: 'darwin' | 'win32' | 'linux' | 'web'
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
  openInternal(path: 'history' | 'bookmarks' | 'extensions' | 'settings/appearance' | 'settings/languages' | 'settings/about'): Promise<void>
  openUtility(path: 'history' | 'bookmarks' | 'extensions' | 'settings/appearance'): Promise<void>
  updateSettings(settings: Partial<BrowserSettings>): Promise<void>
  clearHistory(): Promise<void>
  removeBookmark(id: string): Promise<void>
  createBookmarkFolder(name: string, parentId?: string): Promise<void>
  removeBookmarkFolder(id: string): Promise<void>
  moveBookmark(id: string, parentId?: string): Promise<void>
  showMenu(position: PopupPosition): Promise<void>
  showTabMenu(request: TabMenuRequest): Promise<void>
  showBookmarksBarMenu(position: PopupPosition): Promise<void>
  showBookmarksOverflowMenu(request: BookmarksOverflowRequest): Promise<void>
  showSiteInfo(position: PopupPosition): Promise<void>
  listExtensions(): Promise<BrowserExtension[]>
  installExtension(): Promise<ExtensionOperationResult>
  removeExtension(id: string): Promise<ExtensionOperationResult>
  getAppVersion(): Promise<string>
  checkForUpdates(force?: boolean): Promise<ReleaseStatus>
  openLatestRelease(): Promise<void>
  setContentBounds(bounds: ContentBounds): void
  onState(callback: (state: BrowserState) => void): () => void
}
