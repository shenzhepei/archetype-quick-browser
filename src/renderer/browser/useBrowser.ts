import { useEffect, useMemo, useState } from 'react'
import type { ArchetypeBridge, BrowserState } from '../../shared/browser'
import { demoBridge } from './demoBridge'

const initialState: BrowserState = {
  tabs: [],
  activeTabId: '',
  bookmarks: [],
  history: [],
  settings: { theme: 'system', language: 'en' }
}

export function useBrowser(): { bridge: ArchetypeBridge; state: BrowserState } {
  const bridge = useMemo(() => window.archetype ?? demoBridge, [])
  const [state, setState] = useState(initialState)

  useEffect(() => {
    let active = true
    void bridge.getState().then((next) => active && setState(next))
    const unsubscribe = bridge.onState(setState)
    return () => {
      active = false
      unsubscribe()
    }
  }, [bridge])

  return { bridge, state }
}
