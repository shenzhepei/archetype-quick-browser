import { fireEvent, render, screen } from '@testing-library/react'
import type { ArchetypeBridge, BrowserState } from '../src/shared/browser'
import { Toolbar } from '../src/renderer/browser/Toolbar'
import '../src/renderer/i18n'

const state: BrowserState = {
  tabs: [{ id: 'one', url: 'https://example.com', title: 'Example', loading: false, canGoBack: false, canGoForward: false }],
  activeTabId: 'one',
  bookmarks: [],
  history: [],
  settings: { theme: 'system', language: 'en' },
  siteInfo: { url: 'https://example.com', origin: 'https://example.com', connection: 'secure', permissions: [] }
}

it('requests the native browser menu from the toolbar', () => {
  const bridge = { showMenu: vi.fn(), showSiteInfo: vi.fn(), openUtility: vi.fn() } as unknown as ArchetypeBridge
  render(<Toolbar state={state} bridge={bridge} />)

  fireEvent.click(screen.getByRole('button', { name: 'Profile and settings' }))
  fireEvent.click(screen.getByRole('button', { name: 'Site information' }))
  fireEvent.click(screen.getByRole('button', { name: 'Main menu' }))

  expect(bridge.openUtility).toHaveBeenCalledWith('settings/appearance')
  expect(bridge.showSiteInfo).toHaveBeenCalledWith({ x: 0, y: 0 })
  expect(bridge.showMenu).toHaveBeenCalledWith({ x: 0, y: 0 })
})
