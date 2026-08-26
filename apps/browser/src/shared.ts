import type { ProjectDescriptor, SessionSummary } from '@archetype/protocol'

export type Language = 'en' | 'zh-CN'
export type Theme = 'system' | 'light' | 'dark'

export interface BrowserTab {
  id: string
  title: string
  url: string
  loading: boolean
  canGoBack: boolean
  canGoForward: boolean
}

export interface RuntimeStatus {
  configured: boolean
  project?: ProjectDescriptor
  session?: SessionSummary | null
  error?: string
}

export interface BrowserState {
  tabs: BrowserTab[]
  activeTabId: string
  language: Language
  theme: Theme
  runtime: RuntimeStatus
}

export interface ShellBridge {
  getState(): Promise<BrowserState>
  newTab(url?: string): Promise<void>
  closeTab(id: string): Promise<void>
  selectTab(id: string): Promise<void>
  navigate(input: string): Promise<void>
  back(): Promise<void>
  forward(): Promise<void>
  reload(): Promise<void>
  updatePreferences(value: { language?: Language; theme?: Theme }): Promise<void>
  setContentBounds(bounds: { x: number; y: number; width: number; height: number }): void
  onState(listener: (state: BrowserState) => void): () => void
}
