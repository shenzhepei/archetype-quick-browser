import { useEffect, useRef } from 'react'
import type { ArchetypeBridge, BrowserState } from '../../shared/browser'
import { BookmarksBar } from './BookmarksBar'
import { InternalPage } from './InternalPage'
import { TabStrip } from './TabStrip'
import { Toolbar } from './Toolbar'

interface BrowserShellProps {
  bridge: ArchetypeBridge
  state: BrowserState
}

export function BrowserShell({ bridge, state }: BrowserShellProps): React.JSX.Element {
  const contentRef = useRef<HTMLElement>(null)
  const activeTab = state.tabs.find((tab) => tab.id === state.activeTabId)
  const internal = activeTab?.url.startsWith('archetype://') ?? false

  useEffect(() => {
    const content = contentRef.current
    if (!content) return
    const update = (): void => {
      const rect = content.getBoundingClientRect()
      bridge.setContentBounds({ x: rect.x, y: rect.y, width: rect.width, height: rect.height })
    }
    const observer = new ResizeObserver(update)
    observer.observe(content)
    update()
    return () => observer.disconnect()
  }, [bridge, state.bookmarks.length])

  return (
    <div className={`browser-shell ${state.bookmarks.length > 0 ? 'has-bookmarks' : ''}`}>
      <TabStrip state={state} bridge={bridge} />
      <Toolbar state={state} bridge={bridge} />
      <BookmarksBar bookmarks={state.bookmarks} bridge={bridge} />
      <main className="browser-content" ref={contentRef}>
        {internal && activeTab ? <InternalPage url={activeTab.url} state={state} bridge={bridge} /> : null}
        {!window.archetype && !internal ? <PreviewContent /> : null}
      </main>
    </div>
  )
}

function PreviewContent(): React.JSX.Element {
  return (
    <div className="preview-content">
      <div className="preview-wordmark">Archetype</div>
      <div className="preview-search">Search the web</div>
      <div className="preview-links">
        <span>GitHub</span><span>Documentation</span><span>Release notes</span>
      </div>
    </div>
  )
}
