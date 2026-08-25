import { CheckCircle2, CircleAlert, Clock3, Download, Info, Languages, LoaderCircle, Palette, RefreshCw, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { ArchetypeBridge, BrowserState, Language, ThemePreference } from '../../shared/browser'

export function InternalPage({ url, state, bridge }: { url: string; state: BrowserState; bridge: ArchetypeBridge }): React.JSX.Element {
  if (url.includes('history')) return <HistoryPage state={state} bridge={bridge} />
  const section = url.includes('/about') ? 'about' : url.includes('/languages') ? 'languages' : 'appearance'
  return <SettingsPage section={section} state={state} bridge={bridge} />
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

function SettingsPage({ section, state, bridge }: { section: 'appearance' | 'languages' | 'about'; state: BrowserState; bridge: ArchetypeBridge }): React.JSX.Element {
  const { t } = useTranslation()
  return (
    <div className="settings-layout">
      <aside>
        <h1>{t('settings')}</h1>
        <button className={section === 'appearance' ? 'is-active' : ''} onClick={() => void bridge.openInternal('settings/appearance')}><Palette size={17} />{t('appearance')}</button>
        <button className={section === 'languages' ? 'is-active' : ''} onClick={() => void bridge.openInternal('settings/languages')}><Languages size={17} />{t('language')}</button>
        <button className={section === 'about' ? 'is-active' : ''} onClick={() => void bridge.openInternal('settings/about')}><Info size={17} />{t('about')}</button>
      </aside>
      <section className="settings-content">
        {section === 'appearance' ? <Appearance state={state} bridge={bridge} /> : section === 'languages' ? <LanguageSettings state={state} bridge={bridge} /> : <About bridge={bridge} />}
      </section>
    </div>
  )
}

function LanguageSettings({ state, bridge }: { state: BrowserState; bridge: ArchetypeBridge }): React.JSX.Element {
  const { t } = useTranslation()
  const languages: Language[] = ['en', 'zh-CN']
  return (
    <div className="settings-section">
      <div className="section-heading"><Languages size={22} /><h2>{t('language')}</h2></div>
      <div className="setting-row"><span>{t('language')}</span><div className="segmented">{languages.map((language) => <button key={language} className={state.settings.language === language ? 'is-selected' : ''} onClick={() => void bridge.updateSettings({ language })}>{language === 'en' ? t('english') : t('chinese')}</button>)}</div></div>
    </div>
  )
}

function Appearance({ state, bridge }: { state: BrowserState; bridge: ArchetypeBridge }): React.JSX.Element {
  const { t } = useTranslation()
  const themes: ThemePreference[] = ['system', 'light', 'dark']
  return (
    <div className="settings-section">
      <div className="section-heading"><Palette size={22} /><h2>{t('appearance')}</h2></div>
      <div className="setting-row"><span>{t('theme')}</span><div className="segmented">{themes.map((theme) => <button key={theme} className={state.settings.theme === theme ? 'is-selected' : ''} onClick={() => void bridge.updateSettings({ theme })}>{t(theme)}</button>)}</div></div>
    </div>
  )
}

function About({ bridge }: { bridge: ArchetypeBridge }): React.JSX.Element {
  const { t } = useTranslation()
  const [version, setVersion] = useState('...')
  const [release, setRelease] = useState<Awaited<ReturnType<ArchetypeBridge['checkForUpdates']>>>()
  const [checking, setChecking] = useState(true)

  useEffect(() => {
    let active = true
    void bridge.getAppVersion().then((next) => { if (active) setVersion(next) })
    void bridge.checkForUpdates().then((next) => { if (active) setRelease(next) }).finally(() => { if (active) setChecking(false) })
    return () => { active = false }
  }, [bridge])

  const checkAgain = async (): Promise<void> => {
    setChecking(true)
    try {
      setRelease(await bridge.checkForUpdates(true))
    } finally {
      setChecking(false)
    }
  }

  const status = checking
    ? t('checkingUpdates')
    : release?.state === 'up-to-date'
      ? t('upToDate')
      : release?.state === 'update-available'
        ? t('updateAvailable', { version: release.latestVersion })
        : release?.state === 'no-release'
          ? t('noRelease')
          : t('updateUnavailable')
  const StatusIcon = checking ? LoaderCircle : release?.state === 'up-to-date' ? CheckCircle2 : CircleAlert

  return (
    <div className="settings-section about-section">
      <div className="brand-mark">A</div><h2>Archetype</h2><p>{t('version', { version })}</p><p>{t('chromium')}</p>
      <div className="release-status" aria-live="polite">
        <div className="release-copy"><StatusIcon className={checking ? 'spinner' : ''} size={18} /><span>{status}</span></div>
        <div className="release-actions">
          <button className="command-button" disabled={checking} onClick={() => void checkAgain()}><RefreshCw size={15} />{t('checkAgain')}</button>
          {release?.state === 'update-available' ? <button className="command-button" onClick={() => void bridge.openLatestRelease()}><Download size={15} />{t('viewRelease')}</button> : null}
        </div>
      </div>
    </div>
  )
}
