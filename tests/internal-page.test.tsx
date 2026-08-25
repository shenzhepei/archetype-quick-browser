import { fireEvent, render, screen } from '@testing-library/react'
import type { ArchetypeBridge, BrowserState } from '../src/shared/browser'
import { InternalPage } from '../src/renderer/browser/InternalPage'
import '../src/renderer/i18n'

const bridge = { updateSettings: vi.fn(), clearHistory: vi.fn(), navigate: vi.fn(), openInternal: vi.fn() } as unknown as ArchetypeBridge
const state: BrowserState = { tabs: [], activeTabId: '', bookmarks: [], history: [], settings: { theme: 'system', language: 'en' } }

it('changes the appearance preference', () => {
  render(<InternalPage url="archetype://settings/appearance" state={state} bridge={bridge} />)
  fireEvent.click(screen.getByRole('button', { name: 'Dark' }))
  expect(bridge.updateSettings).toHaveBeenCalledWith({ theme: 'dark' })
})

it('renders an empty history state', () => {
  render(<InternalPage url="archetype://history" state={state} bridge={bridge} />)
  expect(screen.getByText('No browsing history yet')).toBeInTheDocument()
})
