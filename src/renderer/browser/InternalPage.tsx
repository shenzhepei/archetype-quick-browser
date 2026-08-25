import { Clock3, Info, MonitorCog, Palette, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { ArchetypeBridge, BrowserState, ThemePreference } from '../../shared/browser'

export function InternalPage({ url, state, bridge }: { url: string; state: BrowserState; bridge: ArchetypeBridge }): React.JSX.Element {
  if (url.includes('history')) return <HistoryPage state={state} bridge={bridge} />
  return <SettingsPage section={url.includes('/about') ? 'about' : 'appearance'} state={state} bridge={bridge} />
}

function HistoryPage({ state, bridge }: { state: BrowserState; bridge: ArchetypeBridge }): React.JSX.Element {
  const { t } = useTranslation()
  return (
    <section className="internal-page history-page">
      <header><div><Clock3 size={22} /><h1>{t('history')}</h1></div><button className="command-button" disabled={state.history.length === 0} onClick={() => void bridge.clearHistory()}><Trash2 size={16} />{t('clearHistory')}</button></header>
      {state.history.length === 0 ? <div className="empty-state"><Clock3 size={34} /><p>{t('emptyHistory')}</p></div> : (
        <div className="history-list">
          {state.history.map((entry) => (
            <button key={entry.id} onClick={() => void bridge.navigate(entry.url)}>
              <time>{new Date(entry.visitedAt).toLocaleString()}</time><span><strong>{entry.title}</strong><small>{entry.url}</small></span>
            </button>
          ))}
        </div>
      )}
    </section>
  )
}

function SettingsPage({ section, state, bridge }: { section: 'appearance' | 'about'; state: BrowserState; bridge: ArchetypeBridge }): React.JSX.Element {
  const { t } = useTranslation()
  return (
    <div className="settings-layout">
      <aside>
        <h1>{t('settings')}</h1>
        <button className={section === 'appearance' ? 'is-active' : ''} onClick={() => void bridge.openInternal('settings/appearance')}><Palette size={17} />{t('appearance')}</button>
        <button className={section === 'about' ? 'is-active' : ''} onClick={() => void bridge.openInternal('settings/about')}><Info size={17} />{t('about')}</button>
      </aside>
      <section className="settings-content">
        {section === 'appearance' ? <Appearance state={state} bridge={bridge} /> : <About />}
      </section>
    </div>
  )
}

function Appearance({ state, bridge }: { state: BrowserState; bridge: ArchetypeBridge }): React.JSX.Element {
  const { t } = useTranslation()
  const themes: ThemePreference[] = ['system', 'light', 'dark']
  return (
    <div className="settings-section">
      <div className="section-heading"><MonitorCog size={22} /><h2>{t('appearance')}</h2></div>
      <div className="setting-row"><span>{t('theme')}</span><div className="segmented">{themes.map((theme) => <button key={theme} className={state.settings.theme === theme ? 'is-selected' : ''} onClick={() => void bridge.updateSettings({ theme })}>{t(theme)}</button>)}</div></div>
    </div>
  )
}

function About(): React.JSX.Element {
  const { t } = useTranslation()
  return (
    <div className="settings-section about-section">
      <div className="brand-mark">A</div><h2>Archetype</h2><p>{t('version', { version: '0.1.0' })}</p><p>{t('chromium')}</p>
    </div>
  )
}
