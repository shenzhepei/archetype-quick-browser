import { mkdir, readFile, rename, writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import { app } from 'electron'
import type { Bookmark, BookmarkFolder, BrowserSettings, HistoryEntry } from '../shared/browser'

interface PersistedState {
  bookmarks: Bookmark[]
  bookmarkFolders: BookmarkFolder[]
  history: HistoryEntry[]
  settings: BrowserSettings
  tabs: Array<{ url: string; title: string }>
  activeTab: number
  extensions: string[]
}

const defaults: PersistedState = {
  bookmarks: [],
  bookmarkFolders: [],
  history: [],
  settings: { theme: 'system', language: 'en' },
  tabs: [{ url: 'about:blank', title: 'New tab' }],
  activeTab: 0,
  extensions: []
}

export class BrowserStore {
  private readonly path = join(app.getPath('userData'), 'browser-state.json')
  private data: PersistedState = structuredClone(defaults)
  private writeQueue: Promise<void> = Promise.resolve()

  async load(): Promise<void> {
    try {
      const parsed = JSON.parse(await readFile(this.path, 'utf8')) as Partial<PersistedState>
      this.data = {
        bookmarks: Array.isArray(parsed.bookmarks) ? parsed.bookmarks : [],
        bookmarkFolders: Array.isArray(parsed.bookmarkFolders) ? parsed.bookmarkFolders : [],
        history: Array.isArray(parsed.history) ? parsed.history : [],
        settings: {
          theme: ['system', 'light', 'dark'].includes(parsed.settings?.theme ?? '')
            ? parsed.settings!.theme
            : 'system',
          language: parsed.settings?.language === 'zh-CN' ? 'zh-CN' : 'en'
        },
        tabs: Array.isArray(parsed.tabs) && parsed.tabs.length > 0 ? parsed.tabs : defaults.tabs,
        activeTab: typeof parsed.activeTab === 'number' ? parsed.activeTab : 0,
        extensions: Array.isArray(parsed.extensions)
          ? parsed.extensions.filter((path): path is string => typeof path === 'string')
          : []
      }
    } catch {
      this.data = structuredClone(defaults)
    }
  }

  snapshot(): PersistedState {
    return structuredClone(this.data)
  }

  async setExtensionPaths(paths: string[]): Promise<void> {
    await this.update((state) => {
      state.extensions = paths
    })
  }

  async update(update: (state: PersistedState) => void): Promise<void> {
    update(this.data)
    const serialized = JSON.stringify(this.data, null, 2)
    this.writeQueue = this.writeQueue.catch(() => undefined).then(async () => {
      await mkdir(dirname(this.path), { recursive: true })
      const temporary = `${this.path}.tmp`
      await writeFile(temporary, serialized)
      await rename(temporary, this.path)
    })
    await this.writeQueue
  }
}
