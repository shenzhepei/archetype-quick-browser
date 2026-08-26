import { fireEvent, render, screen } from '@testing-library/react'
import type { ArchetypeBridge, BrowserState } from '../src/shared/browser'
import { TabStrip } from '../src/renderer/browser/TabStrip'
import '../src/renderer/i18n'

const state: BrowserState = {
  tabs: [{ id: 'one', url: 'about:blank', title: 'New tab', loading: false, canGoBack: false, canGoForward: false }],
  activeTabId: 'one',
  bookmarks: [],
  bookmarkFolders: [],
  history: [],
  settings: { theme: 'system', language: 'en' },
  siteInfo: { url: '', connection: 'none', permissions: [] }
}

it('selects, closes, and creates tabs', () => {
  const bridge = {
    selectTab: vi.fn(),
    closeTab: vi.fn(),
    newTab: vi.fn(),
    showTabMenu: vi.fn()
  } as unknown as ArchetypeBridge
  const { container } = render(<TabStrip state={state} bridge={bridge} />)
  expect(container.querySelector('.tabs-list')).toContainElement(screen.getByRole('tab'))
  expect(container.querySelector('.tabs-scroll')).toContainElement(screen.getByLabelText('New tab'))
  fireEvent.click(screen.getByRole('tab'))
  fireEvent.click(screen.getByLabelText('Close tab'))
  fireEvent.click(screen.getByLabelText('New tab'))
  fireEvent.contextMenu(screen.getByRole('tab'), { clientX: 120, clientY: 24 })
  expect(bridge.selectTab).toHaveBeenCalledWith('one')
  expect(bridge.closeTab).toHaveBeenCalledWith('one')
  expect(bridge.newTab).toHaveBeenCalled()
  expect(bridge.showTabMenu).toHaveBeenCalledWith({ tabId: 'one', x: 120, y: 24 })
})
