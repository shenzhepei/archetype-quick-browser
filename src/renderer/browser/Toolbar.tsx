import { ArrowLeft, ArrowRight, EllipsisVertical, RefreshCw, Star, UserRound, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { ArchetypeBridge, BrowserState } from '../../shared/browser'

interface ToolbarProps {
  state: BrowserState
  bridge: ArchetypeBridge
}

export function Toolbar({ state, bridge }: ToolbarProps): React.JSX.Element {
  const { t } = useTranslation()
  const active = state.tabs.find((tab) => tab.id === state.activeTabId)
  const [address, setAddress] = useState(active?.url ?? '')
  useEffect(() => setAddress(active?.url === 'about:blank' ? '' : active?.url ?? ''), [active?.url])
  const bookmarked = state.bookmarks.some((bookmark) => bookmark.url === active?.url)

  return (
    <div className="toolbar">
      <button className="icon-button" disabled={!active?.canGoBack} aria-label={t('back')} title={t('back')} onClick={() => void bridge.back()}>
        <ArrowLeft size={18} />
      </button>
      <button className="icon-button" disabled={!active?.canGoForward} aria-label={t('forward')} title={t('forward')} onClick={() => void bridge.forward()}>
        <ArrowRight size={18} />
      </button>
      <button className="icon-button" aria-label={active?.loading ? t('stop') : t('reload')} title={active?.loading ? t('stop') : t('reload')} onClick={() => void (active?.loading ? bridge.stop() : bridge.reload())}>
        {active?.loading ? <X size={17} /> : <RefreshCw size={17} />}
      </button>
      <form
        className="address-bar"
        onSubmit={(event) => {
          event.preventDefault()
          void bridge.navigate(address)
        }}
      >
        <input
          value={address}
          aria-label={t('addressPlaceholder')}
          placeholder={t('addressPlaceholder')}
          spellCheck={false}
          onChange={(event) => setAddress(event.target.value)}
          onFocus={(event) => event.currentTarget.select()}
        />
        <button
          type="button"
          className={`address-action ${bookmarked ? 'is-active' : ''}`}
          aria-label={bookmarked ? t('removeBookmark') : t('bookmark')}
          title={bookmarked ? t('removeBookmark') : t('bookmark')}
          onClick={() => void bridge.toggleBookmark()}
        >
          <Star size={17} fill={bookmarked ? 'currentColor' : 'none'} />
        </button>
      </form>
      <button className="avatar-button" aria-label={t('profile')} title={t('profile')} onClick={() => void bridge.openUtility('settings/appearance')}>
        <UserRound size={17} />
      </button>
      <button
        className="icon-button"
        aria-label={t('menu')}
        title={t('menu')}
        onClick={(event) => {
          const rect = event.currentTarget.getBoundingClientRect()
          void bridge.showMenu({ x: rect.left, y: rect.bottom })
        }}
      >
        <EllipsisVertical size={18} />
      </button>
    </div>
  )
}
