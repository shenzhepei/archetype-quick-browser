import { net, shell } from 'electron'
import { isReleaseNewer } from '../shared/browser'
import type { ReleaseStatus } from '../shared/browser'

const API_URL = 'https://api.github.com/repos/shenzhepei/archetype-quick-browser/releases/latest'
const RELEASE_URL_PREFIX = 'https://github.com/shenzhepei/archetype-quick-browser/releases/tag/'
const CACHE_DURATION_MS = 5 * 60 * 1000

export class ReleaseService {
  private cached?: ReleaseStatus

  constructor(private readonly getCurrentVersion: () => string) {}

  async check(force = false): Promise<ReleaseStatus> {
    if (!force && this.cached && Date.now() - this.cached.checkedAt < CACHE_DURATION_MS) return this.cached
    const currentVersion = this.getCurrentVersion()
    try {
      const response = await net.fetch(API_URL, {
        headers: {
          Accept: 'application/vnd.github+json',
          'User-Agent': 'Archetype-Quick-Browser'
        },
        signal: AbortSignal.timeout(8000)
      })
      if (response.status === 404) return this.store({ currentVersion, checkedAt: Date.now(), state: 'no-release' })
      if (!response.ok) return this.store({ currentVersion, checkedAt: Date.now(), state: 'unavailable' })
      const payload = await response.json() as { tag_name?: unknown; html_url?: unknown }
      if (typeof payload.tag_name !== 'string' || typeof payload.html_url !== 'string' || !payload.html_url.startsWith(RELEASE_URL_PREFIX)) {
        return this.store({ currentVersion, checkedAt: Date.now(), state: 'unavailable' })
      }
      const latestVersion = payload.tag_name.replace(/^v/, '')
      const newer = isReleaseNewer(latestVersion, currentVersion)
      if (newer === undefined) return this.store({ currentVersion, checkedAt: Date.now(), state: 'unavailable' })
      return this.store({
        currentVersion,
        latestVersion,
        releaseUrl: payload.html_url,
        checkedAt: Date.now(),
        state: newer ? 'update-available' : 'up-to-date'
      })
    } catch {
      return this.store({ currentVersion, checkedAt: Date.now(), state: 'unavailable' })
    }
  }

  async openLatest(): Promise<void> {
    if (this.cached?.state === 'update-available' && this.cached.releaseUrl?.startsWith(RELEASE_URL_PREFIX)) {
      await shell.openExternal(this.cached.releaseUrl)
    }
  }

  private store(status: ReleaseStatus): ReleaseStatus {
    this.cached = status
    return status
  }
}
