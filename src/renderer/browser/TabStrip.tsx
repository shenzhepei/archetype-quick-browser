import { Globe2, LoaderCircle, Plus, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { ArchetypeBridge, BrowserState, TabState } from '../../shared/browser'

export function TabStrip({ state, bridge }: { state: BrowserState; bridge: ArchetypeBridge }): React.JSX.Element {
  const { t } = useTranslation()
  return (
    <div className="tab-strip" role="tablist">
      <div className="window-drag-space" />
      <div className="tabs-scroll" style={{ flexBasis: `${state.tabs.length * 221 + 38}px` }}>
        {state.tabs.map((tab) => (
          <button
            className={`tab ${tab.id === state.activeTabId ? 'is-active' : ''}`}
            key={tab.id}
            role="tab"
            aria-selected={tab.id === state.activeTabId}
            onClick={() => void bridge.selectTab(tab.id)}
            onContextMenu={(event) => {
              event.preventDefault()
              void bridge.showTabMenu({ tabId: tab.id, x: event.clientX, y: event.clientY })
            }}
          >
            <TabIcon tab={tab} />
            <span className="tab-title">{tab.title || t('newTab')}</span>
            <span
              className="tab-close"
              role="button"
              tabIndex={0}
              aria-label={t('closeTab')}
              onClick={(event) => {
                event.stopPropagation()
                void bridge.closeTab(tab.id)
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') void bridge.closeTab(tab.id)
              }}
            >
              <X size={13} />
            </span>
          </button>
        ))}
        <button className="icon-button new-tab" aria-label={t('newTab')} title={t('newTab')} onClick={() => void bridge.newTab()}>
          <Plus size={17} />
        </button>
      </div>
      <div className="titlebar-drag-tail" />
    </div>
  )
}

function TabIcon({ tab }: { tab: TabState }): React.JSX.Element {
  const icon = tab.favicon ? <img src={tab.favicon} alt="" /> : <Globe2 size={14} />
  if (!tab.loading) return <span className="favicon">{icon}</span>
  return (
    <span className="loading-favicon">
      <LoaderCircle className="spinner" size={18} />
      <span className="favicon is-loading">{icon}</span>
    </span>
  )
}
