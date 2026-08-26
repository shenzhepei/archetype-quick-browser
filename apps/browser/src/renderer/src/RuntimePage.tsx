import { ServerCog } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { BrowserState } from '../../shared.js'

export function RuntimePage({ state }: { state: BrowserState }): React.JSX.Element {
  const { t } = useTranslation()
  return <div className="runtime-page">
    <header>
      <div className="runtime-mark"><ServerCog size={24} /></div>
      <div><h1>Archetype Runtime</h1><p>{state.runtime.configured ? t('runtimeReady') : t('runtimeMissing')}</p></div>
    </header>
    <div className="status-grid">
      <section><span>{t('project')}</span><strong>{state.runtime.project?.name ?? t('noProject')}</strong><small>{state.runtime.project?.projectId ?? state.runtime.error ?? t('openConfiguredSite')}</small></section>
      <section><span>{t('operations')}</span><strong>{state.runtime.project?.operations.length ?? 0}</strong><small>{state.runtime.project?.operations.map((operation) => operation.name).join(', ') || t('noOperations')}</small></section>
      <section><span>{t('identity')}</span><strong>{state.runtime.session?.displayName ?? state.runtime.session?.subject ?? t('signedOut')}</strong><small>{state.runtime.session?.authenticated ? t('originSession') : t('authOnDemand')}</small></section>
    </div>
    <div className="boundary-note"><strong>{t('trustBoundary')}</strong><p>{t('trustBoundaryDescription')}</p></div>
  </div>
}
