import { History, Languages, Settings } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { ArchetypeBridge, BrowserSettings, Language } from '../../shared/browser'
import { setLanguage } from '../i18n'

export function BrowserMenu({ bridge, settings, close }: { bridge: ArchetypeBridge; settings: BrowserSettings; close(): void }): React.JSX.Element {
  const { t } = useTranslation()
  const chooseLanguage = (language: Language): void => {
    void setLanguage(language)
    void bridge.updateSettings({ language })
  }
  return (
    <div className="browser-menu" role="menu">
      <button role="menuitem" onClick={() => { void bridge.openInternal('history'); close() }}><History size={17} />{t('history')}</button>
      <button role="menuitem" onClick={() => { void bridge.openInternal('settings/appearance'); close() }}><Settings size={17} />{t('settings')}</button>
      <div className="menu-separator" />
      <div className="language-row"><Languages size={17} /><span>{t('language')}</span></div>
      <div className="segmented compact" aria-label={t('language')}>
        <button className={settings.language === 'en' ? 'is-selected' : ''} onClick={() => chooseLanguage('en')}>{t('english')}</button>
        <button className={settings.language === 'zh-CN' ? 'is-selected' : ''} onClick={() => chooseLanguage('zh-CN')}>{t('chinese')}</button>
      </div>
    </div>
  )
}
