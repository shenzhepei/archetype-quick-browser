import { Bookmark as BookmarkIcon, CheckCircle2, CircleAlert, Clock3, Download, Folder, FolderOpen, FolderPlus, Info, Languages, LoaderCircle, PackageOpen, Palette, Puzzle, RefreshCw, Trash2, Upload, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { buildBookmarkTree } from '../../shared/bookmark-tree'
import type { BookmarkFolderNode } from '../../shared/bookmark-tree'
import type { ArchetypeBridge, BrowserExtension, BrowserState, Language, ThemePreference } from '../../shared/browser'
import { BookmarkFavicon } from './BookmarkFavicon'

export function InternalPage({ url, state, bridge }: { url: string; state: BrowserState; bridge: ArchetypeBridge }): React.JSX.Element {
  if (url.includes('history')) return <HistoryPage state={state} bridge={bridge} />
  if (url.includes('bookmarks')) return <BookmarksPage url={url} state={state} bridge={bridge} />
  if (url.includes('extensions')) return <ExtensionsPage bridge={bridge} />
  const section = url.includes('/about') ? 'about' : url.includes('/languages') ? 'languages' : 'appearance'
  return <SettingsPage section={section} state={state} bridge={bridge} />
}

function BookmarksPage({ url, state, bridge }: { url: string; state: BrowserState; bridge: ArchetypeBridge }): React.JSX.Element {
  const { t } = useTranslation()
  const [selectedFolderId, setSelectedFolderId] = useState<string>()
  const [creatingFolder, setCreatingFolder] = useState(url.includes('/new-folder'))
  const [folderName, setFolderName] = useState('')
  const tree = buildBookmarkTree(state.bookmarks, state.bookmarkFolders)
  const selectedFolder = state.bookmarkFolders.find((folder) => folder.id === selectedFolderId)
  const currentFolders = treeNode(tree.folders, selectedFolderId)?.folders ?? tree.folders
  const currentBookmarks = treeNode(tree.folders, selectedFolderId)?.bookmarks ?? tree.bookmarks
  const folderOptions = flattenFolders(tree.folders)

  useEffect(() => {
    if (selectedFolderId && !state.bookmarkFolders.some((folder) => folder.id === selectedFolderId)) {
      setSelectedFolderId(undefined)
    }
  }, [selectedFolderId, state.bookmarkFolders])

  useEffect(() => {
    if (url.includes('/new-folder')) {
      setSelectedFolderId(undefined)
      setCreatingFolder(true)
    }
  }, [url])

  const createFolder = (): void => {
    const name = folderName.trim()
    if (!name) return
    void bridge.createBookmarkFolder(name, selectedFolderId)
    setFolderName('')
    setCreatingFolder(false)
  }

  const removeFolder = (id: string, name: string): void => {
    if (window.confirm(t('deleteFolderConfirmation', { name }))) void bridge.removeBookmarkFolder(id)
  }

  return (
    <section className="internal-page bookmarks-page">
      <header>
        <div><BookmarkIcon size={22} /><h1>{t('bookmarks')}</h1></div>
        <button className="command-button" onClick={() => setCreatingFolder(true)}><FolderPlus size={16} />{t('newFolder')}</button>
      </header>
      <div className="bookmark-manager">
        <aside className="bookmark-tree" aria-label={t('bookmarkFolders')}>
          <button className={!selectedFolderId ? 'is-active' : ''} onClick={() => setSelectedFolderId(undefined)}><FolderOpen size={17} />{t('rootBookmarks')}</button>
          {tree.folders.map((folder) => <FolderTreeItem key={folder.id} folder={folder} selectedId={selectedFolderId} depth={0} onSelect={setSelectedFolderId} />)}
        </aside>
        <div className="bookmark-directory">
          <div className="bookmark-directory-heading">
            <div><FolderOpen size={19} /><h2>{selectedFolder?.name ?? t('rootBookmarks')}</h2></div>
            {selectedFolder ? <button className="icon-button" aria-label={t('deleteFolderNamed', { name: selectedFolder.name })} title={t('deleteFolder')} onClick={() => removeFolder(selectedFolder.id, selectedFolder.name)}><Trash2 size={16} /></button> : null}
          </div>
          {creatingFolder ? (
            <form className="new-folder-form" onSubmit={(event) => { event.preventDefault(); createFolder() }}>
              <input autoFocus maxLength={80} value={folderName} placeholder={t('folderName')} aria-label={t('folderName')} onChange={(event) => setFolderName(event.target.value)} />
              <button className="command-button" type="submit" disabled={!folderName.trim()}>{t('create')}</button>
              <button className="icon-button" type="button" aria-label={t('cancel')} title={t('cancel')} onClick={() => { setCreatingFolder(false); setFolderName('') }}><X size={16} /></button>
            </form>
          ) : null}
          {currentFolders.length === 0 && currentBookmarks.length === 0 ? <div className="empty-state bookmark-empty"><Folder size={34} /><p>{t('emptyFolder')}</p></div> : (
            <div className="bookmark-list">
              {currentFolders.map((folder) => (
                <article className="bookmark-folder-row" key={folder.id}>
                  <Folder size={20} />
                  <button className="folder-open" onClick={() => setSelectedFolderId(folder.id)}><strong>{folder.name}</strong><small>{t('folderItemCount', { count: folder.folders.length + folder.bookmarks.length })}</small></button>
                  <button className="icon-button" aria-label={t('deleteFolderNamed', { name: folder.name })} title={t('deleteFolder')} onClick={() => removeFolder(folder.id, folder.name)}><Trash2 size={16} /></button>
                </article>
              ))}
              {[...currentBookmarks].sort((left, right) => right.createdAt - left.createdAt).map((bookmark) => (
                <article key={bookmark.id}>
                  <BookmarkFavicon favicon={bookmark.favicon} size={20} />
                  <button className="bookmark-open" onClick={() => void bridge.navigate(bookmark.url)}>
                    <time>{new Date(bookmark.createdAt).toLocaleString()}</time>
                    <span><strong>{bookmark.title}</strong><small>{bookmark.url}</small></span>
                  </button>
                  <label className="bookmark-destination"><span>{t('moveTo')}</span><select aria-label={t('moveBookmarkNamed', { title: bookmark.title })} value={bookmark.parentId ?? ''} onChange={(event) => void bridge.moveBookmark(bookmark.id, event.target.value || undefined)}><option value="">{t('rootBookmarks')}</option>{folderOptions.map(({ folder, depth }) => <option key={folder.id} value={folder.id}>{`${'  '.repeat(depth)}${folder.name}`}</option>)}</select></label>
                  <button className="icon-button" aria-label={t('removeBookmarkNamed', { title: bookmark.title })} title={t('remove')} onClick={() => void bridge.removeBookmark(bookmark.id)}><Trash2 size={16} /></button>
                </article>
              ))}
            </div>
          )}
        </div>
      </div>
    </section>
  )
}

function treeNode(folders: BookmarkFolderNode[], id?: string): BookmarkFolderNode | undefined {
  if (!id) return undefined
  for (const folder of folders) {
    if (folder.id === id) return folder
    const child = treeNode(folder.folders, id)
    if (child) return child
  }
  return undefined
}

function flattenFolders(folders: BookmarkFolderNode[], depth = 0): Array<{ folder: BookmarkFolderNode; depth: number }> {
  return folders.flatMap((folder) => [{ folder, depth }, ...flattenFolders(folder.folders, depth + 1)])
}

function FolderTreeItem({ folder, selectedId, depth, onSelect }: { folder: BookmarkFolderNode; selectedId?: string; depth: number; onSelect: (id: string) => void }): React.JSX.Element {
  return (
    <>
      <button className={selectedId === folder.id ? 'is-active' : ''} style={{ paddingLeft: 12 + depth * 18 }} onClick={() => onSelect(folder.id)}><Folder size={17} />{folder.name}</button>
      {folder.folders.map((child) => <FolderTreeItem key={child.id} folder={child} selectedId={selectedId} depth={depth + 1} onSelect={onSelect} />)}
    </>
  )
}

function ExtensionsPage({ bridge }: { bridge: ArchetypeBridge }): React.JSX.Element {
  const { t } = useTranslation()
  const [extensions, setExtensions] = useState<BrowserExtension[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(false)

  useEffect(() => {
    let active = true
    void bridge.listExtensions()
      .then((items) => { if (active) setExtensions(items) })
      .catch(() => { if (active) setError(true) })
      .finally(() => { if (active) setLoading(false) })
    return () => { active = false }
  }, [bridge])

  const install = async (): Promise<void> => {
    setError(false)
    try {
      const result = await bridge.installExtension()
      setExtensions(result.extensions)
      if (!result.ok) setError(true)
    } catch {
      setError(true)
    }
  }

  const remove = async (id: string): Promise<void> => {
    setError(false)
    try {
      const result = await bridge.removeExtension(id)
      setExtensions(result.extensions)
      if (!result.ok) setError(true)
    } catch {
      setError(true)
    }
  }

  return (
    <section className="internal-page extensions-page">
      <header>
        <div><Puzzle size={22} /><h1>{t('extensions')}</h1></div>
        <button className="command-button" onClick={() => void install()}><Upload size={16} />{t('loadUnpacked')}</button>
      </header>
      {error ? <div className="inline-error" role="alert"><CircleAlert size={17} />{t('extensionOperationFailed')}</div> : null}
      {loading ? <div className="empty-state"><LoaderCircle className="spinner" size={30} /></div> : extensions.length === 0 ? (
        <div className="empty-state"><PackageOpen size={34} /><p>{t('emptyExtensions')}</p></div>
      ) : (
        <div className="extension-list">
          {extensions.map((extension) => (
            <article className="extension-item" key={extension.id}>
              <Puzzle size={22} />
              <div className="extension-copy">
                <div><strong>{extension.name}</strong><span>{t('extensionVersion', { version: extension.version })}</span></div>
                {extension.description ? <p>{extension.description}</p> : null}
                <small>{extension.path}</small>
              </div>
              <button className="icon-button" aria-label={t('removeExtension', { name: extension.name })} title={t('remove')} onClick={() => void remove(extension.id)}><Trash2 size={16} /></button>
            </article>
          ))}
        </div>
      )}
    </section>
  )
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
