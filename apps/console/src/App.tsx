import { useEffect, useMemo, useState } from 'react'
import { Activity, Database, FolderKanban, LogOut, Plus, RefreshCw, ShieldCheck, Users } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { api, type AuditEntry, type Member, type Project, type Role, type Session } from './api.js'
import { languageKey, type ConsoleLanguage } from './i18n.js'

const mutableSettings = new Set<Role>(['owner', 'admin', 'developer', 'operator'])
const memberManagers = new Set<Role>(['owner', 'admin'])

export function App(): React.JSX.Element {
  const { t, i18n } = useTranslation()
  const [session, setSession] = useState<Session | null>()
  const [projects, setProjects] = useState<Project[]>([])
  const [selectedId, setSelectedId] = useState('')
  const [audit, setAudit] = useState<AuditEntry[]>([])
  const [members, setMembers] = useState<Member[]>([])
  const [notice, setNotice] = useState('')
  const selected = useMemo(() => projects.find((project) => project.id === selectedId), [projects, selectedId])
  const organization = session?.organizations[0]

  const load = async (): Promise<void> => {
    try {
      const current = await api<Session>('/v1/control/session')
      const [next, organizationMembers] = await Promise.all([
        api<Project[]>('/v1/control/projects'),
        current.organizations[0] ? api<Member[]>(`/v1/control/organizations/${current.organizations[0].id}/members`) : Promise.resolve([])
      ])
      setSession(current); setProjects(next); setMembers(organizationMembers); setSelectedId((value) => value || next[0]?.id || '')
    } catch (error) {
      if ((error as { status?: number }).status === 401) setSession(null)
      else setNotice(error instanceof Error ? error.message : t('error'))
    }
  }

  useEffect(() => { void load() }, [])
  useEffect(() => { document.documentElement.lang = i18n.language; document.title = `Archetype ${t('control')}` }, [i18n.language, t])

  const mutate = async (action: () => Promise<unknown>): Promise<void> => {
    setNotice('')
    try { await action(); setNotice(t('saved')); await load() } catch (error) { setNotice(error instanceof Error ? error.message : t('error')) }
  }
  const form = (event: React.FormEvent<HTMLFormElement>): FormData => { event.preventDefault(); return new FormData(event.currentTarget) }
  const setLanguage = (language: ConsoleLanguage): void => { localStorage.setItem(languageKey, language); void i18n.changeLanguage(language) }

  if (session === undefined) return <main className="centered"><span className="loader" /></main>
  if (session === null) return <main className="login-view"><div className="language-switch"><button onClick={() => setLanguage('en')}>English</button><button onClick={() => setLanguage('zh-CN')}>简体中文</button></div><section><ShieldCheck size={32} /><h1>{t('signInRequired')}</h1><p>{t('signInHint')}</p><a className="primary" href="/v1/control/auth/login?returnTo=/console/">{t('signIn')}</a></section></main>

  return <main className="console-shell">
    <header><div className="brand"><ShieldCheck size={22} /><strong>Archetype</strong><span>{t('control')}</span></div><div className="header-actions"><span>{session.displayName ?? session.subject}</span><button onClick={() => setLanguage(i18n.language === 'en' ? 'zh-CN' : 'en')}>{i18n.language === 'en' ? '简体中文' : 'English'}</button><button className="icon" title={t('signOut')} onClick={() => void api('/v1/control/session', { method: 'DELETE' }).then(() => setSession(null))}><LogOut size={17} /></button></div></header>
    <aside><div className="org"><small>{t('organization')}</small><strong>{organization?.name}</strong><span>{t(organization?.role ?? 'auditor')}</span></div><div className="aside-title"><span>{t('projects')}</span>{organization && memberManagers.has(organization.role) || organization?.role === 'developer' ? <button className="icon" title={t('createProject')} onClick={() => document.getElementById('create-project')?.showPopover()}><Plus size={16} /></button> : null}</div><nav>{projects.map((project) => <button className={project.id === selectedId ? 'active' : ''} key={project.id} onClick={() => { setSelectedId(project.id); setAudit([]) }}><FolderKanban size={16} /><span><strong>{project.name}</strong><small>{project.deployedAt ? t('deployed') : t('notDeployed')}</small></span></button>)}</nav></aside>
    <section className="workspace">{notice && <div className="notice">{notice}</div>}{selected ? <><div className="workspace-heading"><div><h1>{selected.name}</h1><p>{selected.id}</p></div><span className="role-badge">{t(selected.role)}</span></div><div className="settings-grid">
      <SettingsSection icon={<Activity size={18} />} title={t('origins')}><ul className="values">{selected.allowedOrigins.map((origin) => <li key={origin}>{origin}</li>)}</ul>{mutableSettings.has(selected.role) && <form onSubmit={(event) => { const data = form(event); void mutate(() => api(`/v1/control/projects/${selected.id}/origins`, { method: 'POST', body: JSON.stringify({ origin: data.get('origin') }) })) }}><input name="origin" type="url" placeholder={t('origin')} required /><button>{t('add')}</button></form>}</SettingsSection>
      <SettingsSection icon={<Database size={18} />} title={t('database')}><p className="state">{selected.hasDatabase ? t('databaseStored') : t('notDeployed')}</p>{mutableSettings.has(selected.role) && <form onSubmit={(event) => { const data = form(event); void mutate(() => api(`/v1/control/projects/${selected.id}/database`, { method: 'PUT', body: JSON.stringify({ dialect: data.get('dialect'), databaseUrl: data.get('databaseUrl') }) })) }}><select name="dialect"><option value="postgres">PostgreSQL</option><option value="mysql">MySQL</option></select><input name="databaseUrl" type="password" autoComplete="new-password" placeholder={t('databaseUrl')} required /><button>{t('saveDatabase')}</button></form>}</SettingsSection>
      <SettingsSection icon={<ShieldCheck size={18} />} title={t('oidc')}><p className="state">{selected.oidc?.issuer ?? '—'}</p>{mutableSettings.has(selected.role) && <form onSubmit={(event) => { const data = form(event); void mutate(() => api(`/v1/control/projects/${selected.id}/oidc`, { method: 'PUT', body: JSON.stringify({ issuer: data.get('issuer'), clientId: data.get('clientId'), clientSecret: data.get('clientSecret') || undefined }) })) }}><input name="issuer" type="url" placeholder={t('issuer')} required /><input name="clientId" placeholder={t('clientId')} required /><input name="clientSecret" type="password" autoComplete="new-password" placeholder={t('clientSecret')} /><button>{t('saveOidc')}</button></form>}</SettingsSection>
      {memberManagers.has(selected.role) && organization && <SettingsSection icon={<Users size={18} />} title={t('members')}><div className="member-list">{members.map((member) => <div key={member.subject}><span><strong>{member.displayName ?? member.subject}</strong><small>{member.subject}</small></span><em>{t(member.role)}</em></div>)}</div><form onSubmit={(event) => { const data = form(event); void mutate(() => api(`/v1/control/organizations/${organization.id}/members`, { method: 'POST', body: JSON.stringify({ subject: data.get('subject'), displayName: data.get('displayName') || undefined, role: data.get('role') }) })) }}><input name="subject" placeholder={t('subject')} required /><input name="displayName" placeholder={t('displayName')} /><select name="role">{(['owner', 'admin', 'developer', 'operator', 'auditor'] as Role[]).map((role) => <option key={role} value={role}>{t(role)}</option>)}</select><button>{t('saveMember')}</button></form></SettingsSection>}
      <SettingsSection icon={<Activity size={18} />} title={t('audit')} action={<button className="icon" title={t('refresh')} onClick={() => void api<AuditEntry[]>(`/v1/control/projects/${selected.id}/logs`).then(setAudit)}><RefreshCw size={15} /></button>}><div className="audit-list">{audit.map((entry, index) => <div key={`${entry.created_at}-${index}`}><strong>{entry.event}</strong><time>{new Date(entry.created_at).toLocaleString()}</time></div>)}</div></SettingsSection>
    </div></> : <div className="empty">{t(projects.length ? 'selectProject' : 'noProjects')}</div>}</section>
    <form id="create-project" popover="auto" className="popover" onSubmit={(event) => { const data = form(event); if (organization) void mutate(() => api('/v1/control/projects', { method: 'POST', body: JSON.stringify({ organizationId: organization.id, name: data.get('name') }) })).then(() => document.getElementById('create-project')?.hidePopover()) }}><h2>{t('createProject')}</h2><input name="name" placeholder={t('projectName')} required /><button>{t('create')}</button></form>
  </main>
}

function SettingsSection({ icon, title, action, children }: { icon: React.ReactNode; title: string; action?: React.ReactNode; children: React.ReactNode }): React.JSX.Element {
  return <section className="settings-section"><header>{icon}<h2>{title}</h2>{action}</header><div>{children}</div></section>
}
