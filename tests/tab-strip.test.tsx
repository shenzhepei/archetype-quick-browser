import { fireEvent, render, screen } from '@testing-library/react'
import type { ArchetypeBridge, BrowserState } from '../src/shared/browser'
import { TabStrip } from '../src/renderer/browser/TabStrip'
import '../src/renderer/i18n'

const state: BrowserState = {
  tabs: [{ id: 'one', url: 'about:blank', title: 'New tab', loading: false, canGoBack: false, canGoForward: false }],
  activeTabId: 'one',
  bookmarks: [],
  history: [],
  settings: { theme: 'system', language: 'en' }
}

it('selects, closes, and creates tabs', () => {
  const bridge = {
    selectTab: vi.fn(),
    closeTab: vi.fn(),
    newTab: vi.fn()
  } as unknown as ArchetypeBridge
  render(<TabStrip state={state} bridge={bridge} />)
  fireEvent.click(screen.getByRole('tab'))
  fireEvent.click(screen.getByLabelText('Close tab'))
  fireEvent.click(screen.getByLabelText('New tab'))
  expect(bridge.selectTab).toHaveBeenCalledWith('one')
  expect(bridge.closeTab).toHaveBeenCalledWith('one')
  expect(bridge.newTab).toHaveBeenCalled()
})
