import { useEffect, useMemo, useRef, useState } from 'react'
import { ArrowLeft, ArrowRight, Globe2, Moon, Plus, RefreshCw, ServerCog, Sun, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { BrowserState } from '../../shared.js'
import { persistPreferences, readPreferences } from './preferences.js'
import { RuntimePage } from './RuntimePage.js'

const initial: BrowserState = {
  tabs: [{ id: 'preview', title: 'Runtime', url: 'archetype://runtime', loading: false, canGoBack: false, canGoForward: false }],
  activeTabId: 'preview',
  language: 'en',
  theme: 'system',
  runtime: { configured: false }
}

export function App(): React.JSX.Element {
  const bridge = window.archetypeShell
  const [state, setState] = useState(initial)
  const active = useMemo(() => state.tabs.find((tab) => tab.id === state.activeTabId) ?? state.tabs[0]!, [state])
  const [address, setAddress] = useState(active.url)
  const contentRef = useRef<HTMLDivElement>(null)
  const { t, i18n } = useTranslation()

  useEffect(() => {
    const preferences = readPreferences(localStorage)
    const unsubscribe = bridge.onState(setState)
    void bridge.getState().then((current) => {
      setState({ ...current, ...preferences })
      if (preferences.language || preferences.theme) void bridge.updatePreferences(preferences)
    })
    return unsubscribe
  }, [bridge])

  useEffect(() => setAddress(active.url), [active.url])

  useEffect(() => {
    void i18n.changeLanguage(state.language)
    document.documentElement.lang = state.language
  }, [i18n, state.language])

  useEffect(() => {
    document.documentElement.dataset.theme = state.theme
  }, [state.theme])

  useEffect(() => {
    document.title = t('documentTitle')
    document.querySelector('meta[name="description"]')?.setAttribute('content', t('documentDescription'))
  }, [state.language, t])

  const updatePreferences = (value: { language?: BrowserState['language']; theme?: BrowserState['theme'] }): void => {
    persistPreferences(localStorage, value)
    void bridge.updatePreferences(value)
  }

  useEffect(() => {
    const element = contentRef.current
    if (!element) return
    const publish = (): void => {
      const rect = element.getBoundingClientRect()
      bridge.setContentBounds({ x: rect.x, y: rect.y, width: rect.width, height: rect.height })
    }
    const observer = new ResizeObserver(publish)
    observer.observe(element)
    publish()
    return () => observer.disconnect()
  }, [bridge])

  return <main className="browser-shell">
    <div className="title-drag"><span>Archetype Runtime</span></div>
    <nav className="tab-strip" aria-label={t('tabs')}>
      <div className="tabs">
        {state.tabs.map((tab) => <button className={`tab ${tab.id === state.activeTabId ? 'active' : ''}`} key={tab.id} onClick={() => void bridge.selectTab(tab.id)}>
          {tab.loading ? <span className="spinner" /> : tab.url.startsWith('archetype:') ? <ServerCog size={15} /> : <Globe2 size={15} />}
          <span className="tab-title">{tab.title}</span>
          <span className="tab-close" role="button" aria-label={t('closeTab', { title: tab.title })} onClick={(event) => { event.stopPropagation(); void bridge.closeTab(tab.id) }}><X size={14} /></span>
        </button>)}
      </div>
      <button className="icon-button" title={t('newTab')} aria-label={t('newTab')} onClick={() => void bridge.newTab()}><Plus size={17} /></button>
    </nav>
    <div className="toolbar">
      <button className="icon-button" disabled={!active.canGoBack} title={t('back')} aria-label={t('back')} onClick={() => void bridge.back()}><ArrowLeft size={18} /></button>
      <button className="icon-button" disabled={!active.canGoForward} title={t('forward')} aria-label={t('forward')} onClick={() => void bridge.forward()}><ArrowRight size={18} /></button>
      <button className="icon-button" title={t('reload')} aria-label={t('reload')} onClick={() => void bridge.reload()}><RefreshCw size={17} /></button>
      <form className="address-form" onSubmit={(event) => { event.preventDefault(); void bridge.navigate(address) }}>
        <span className={`runtime-dot ${state.runtime.configured ? 'connected' : ''}`} title={state.runtime.configured ? t('runtimeReady') : t('runtimeMissing')} />
        <input value={address} onChange={(event) => setAddress(event.target.value)} aria-label={t('address')} spellCheck={false} />
      </form>
      <div className="toolbar-divider" />
      <button className={`language-button ${state.language === 'en' ? 'active' : ''}`} onClick={() => updatePreferences({ language: 'en' })}>English</button>
      <button className={`language-button ${state.language === 'zh-CN' ? 'active' : ''}`} onClick={() => updatePreferences({ language: 'zh-CN' })}>简体中文</button>
      <button className="icon-button" title={t('appearance')} aria-label={t('appearance')} onClick={() => updatePreferences({ theme: state.theme === 'dark' ? 'light' : 'dark' })}>
        {state.theme === 'dark' ? <Sun size={17} /> : <Moon size={17} />}
      </button>
    </div>
    <section className="content" ref={contentRef}>
      {active.url.startsWith('archetype:') && <RuntimePage state={state} />}
    </section>
  </main>
}
