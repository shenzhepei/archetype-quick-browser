import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { BrowserShell } from '../browser/BrowserShell'
import { setLanguage } from '../i18n'
import { useBrowser } from '../browser/useBrowser'

export function App(): React.JSX.Element {
  const { bridge, state } = useBrowser()
  const { t } = useTranslation()

  useEffect(() => {
    document.documentElement.dataset.theme = state.settings.theme
  }, [state.settings.theme])

  useEffect(() => {
    void setLanguage(state.settings.language)
  }, [state.settings.language])

  useEffect(() => {
    document.title = t('appName')
  }, [t])

  return <BrowserShell bridge={bridge} state={state} />
}
