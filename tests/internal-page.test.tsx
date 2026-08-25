import { fireEvent, render, screen } from '@testing-library/react'
import type { ArchetypeBridge, BrowserState } from '../src/shared/browser'
import { InternalPage } from '../src/renderer/browser/InternalPage'
import '../src/renderer/i18n'

const bridge = {
  updateSettings: vi.fn(),
  clearHistory: vi.fn(),
  navigate: vi.fn(),
  openInternal: vi.fn(),
  getAppVersion: vi.fn().mockResolvedValue('1.2.3'),
  checkForUpdates: vi.fn().mockResolvedValue({ currentVersion: '1.2.3', latestVersion: '1.2.4', releaseUrl: 'https://github.com/shenzhepei/archetype-quick-browser/releases/tag/v1.2.4', checkedAt: 0, state: 'update-available' }),
  openLatestRelease: vi.fn()
} as unknown as ArchetypeBridge
const state: BrowserState = { tabs: [], activeTabId: '', bookmarks: [], history: [], settings: { theme: 'system', language: 'en' }, siteInfo: { url: '', connection: 'none', permissions: [] } }

it('changes the appearance preference', () => {
  render(<InternalPage url="archetype://settings/appearance" state={state} bridge={bridge} />)
  fireEvent.click(screen.getByRole('button', { name: 'Dark' }))
  expect(bridge.updateSettings).toHaveBeenCalledWith({ theme: 'dark' })
})

it('changes the language from settings', () => {
  render(<InternalPage url="archetype://settings/languages" state={state} bridge={bridge} />)
  fireEvent.click(screen.getByRole('button', { name: '简体中文' }))
  expect(bridge.updateSettings).toHaveBeenCalledWith({ language: 'zh-CN' })
})

it('renders an empty history state', () => {
  render(<InternalPage url="archetype://history" state={state} bridge={bridge} />)
  expect(screen.getByText('No browsing history yet')).toBeInTheDocument()
})

it('shows the packaged and latest release versions', async () => {
  render(<InternalPage url="archetype://settings/about" state={state} bridge={bridge} />)
  expect(await screen.findByText('Version 1.2.3')).toBeInTheDocument()
  expect(await screen.findByText('Version 1.2.4 is available.')).toBeInTheDocument()
  fireEvent.click(screen.getByRole('button', { name: 'View release' }))
  expect(bridge.openLatestRelease).toHaveBeenCalled()
})
