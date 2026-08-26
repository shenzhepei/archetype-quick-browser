import { BaseWindow, dialog } from 'electron'
import type { Session } from 'electron'
import type { BrowserExtension, ExtensionOperationResult, Language } from '../shared/browser'
import { normalizeExtensionPaths } from './extension-paths'
import { BrowserStore } from './store'

export class ExtensionService {
  constructor(
    private readonly browserSession: Session,
    private readonly store: BrowserStore
  ) {}

  async initialize(): Promise<void> {
    const restored: string[] = []
    for (const path of normalizeExtensionPaths(this.store.snapshot().extensions)) {
      try {
        await this.browserSession.extensions.loadExtension(path, { allowFileAccess: true })
        restored.push(path)
      } catch {
        // Invalid or removed extension directories must not block browser startup.
      }
    }
    await this.store.setExtensionPaths(restored)
  }

  list(): BrowserExtension[] {
    return this.browserSession.extensions.getAllExtensions().map((extension) => ({
      id: extension.id,
      name: extension.name,
      version: extension.version,
      description: typeof extension.manifest?.description === 'string' ? extension.manifest.description : undefined,
      path: extension.path
    }))
  }

  async install(window: BaseWindow, language: Language): Promise<ExtensionOperationResult> {
    const result = await dialog.showOpenDialog(window, {
      title: language === 'zh-CN' ? '加载已解压的扩展程序' : 'Load unpacked extension',
      properties: ['openDirectory']
    })
    if (result.canceled || !result.filePaths[0]) return { ok: true, canceled: true, extensions: this.list() }
    try {
      const extension = await this.browserSession.extensions.loadExtension(result.filePaths[0], { allowFileAccess: true })
      const paths = normalizeExtensionPaths([...this.store.snapshot().extensions, extension.path])
      await this.store.setExtensionPaths(paths)
      return { ok: true, extensions: this.list() }
    } catch {
      return { ok: false, extensions: this.list() }
    }
  }

  async remove(id: string): Promise<ExtensionOperationResult> {
    const extension = this.browserSession.extensions.getExtension(id)
    if (!extension) return { ok: false, extensions: this.list() }
    this.browserSession.extensions.removeExtension(id)
    await this.store.setExtensionPaths(
      this.store.snapshot().extensions.filter((path) => path !== extension.path)
    )
    return { ok: true, extensions: this.list() }
  }
}
