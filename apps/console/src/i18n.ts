import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'

export type ConsoleLanguage = 'en' | 'zh-CN'
export const languageKey = 'archetype.console.language'

export function initialLanguage(storage: Pick<Storage, 'getItem'>): ConsoleLanguage {
  return storage.getItem(languageKey) === 'zh-CN' ? 'zh-CN' : 'en'
}

void i18n.use(initReactI18next).init({
  lng: initialLanguage(typeof localStorage === 'undefined' ? { getItem: () => null } : localStorage), fallbackLng: 'en', interpolation: { escapeValue: false },
  resources: {
    en: { translation: {
      control: 'Control', signIn: 'Sign in with enterprise identity', signInRequired: 'Administrator sign-in required', signInHint: 'Continue through your organization identity provider.',
      projects: 'Projects', createProject: 'Create project', projectName: 'Project name', create: 'Create', noProjects: 'No projects yet.',
      organization: 'Organization', role: 'Role', signedInAs: 'Signed in as', signOut: 'Sign out', settings: 'Project settings',
      origins: 'Allowed origins', origin: 'HTTPS origin', add: 'Add', database: 'Database binding', dialect: 'Dialect', databaseUrl: 'Database URL', saveDatabase: 'Save database', databaseStored: 'Credential stored encrypted',
      oidc: 'Application OIDC', issuer: 'Issuer URL', clientId: 'Client ID', clientSecret: 'Client secret (optional)', saveOidc: 'Save OIDC',
      members: 'Members', subject: 'OIDC subject', displayName: 'Display name', saveMember: 'Add or update member',
      audit: 'Audit log', refresh: 'Refresh', selectProject: 'Select a project', deployed: 'Deployed', notDeployed: 'Not deployed', error: 'Request failed', saved: 'Saved',
      owner: 'Owner', admin: 'Admin', developer: 'Developer', operator: 'Operator', auditor: 'Auditor'
    } },
    'zh-CN': { translation: {
      control: '企业控制台', signIn: '使用企业身份登录', signInRequired: '需要管理员登录', signInHint: '请通过企业身份提供商继续。',
      projects: '项目', createProject: '创建项目', projectName: '项目名称', create: '创建', noProjects: '暂无项目。',
      organization: '组织', role: '角色', signedInAs: '当前身份', signOut: '退出登录', settings: '项目设置',
      origins: '允许的 Origin', origin: 'HTTPS Origin', add: '添加', database: '数据库绑定', dialect: '数据库类型', databaseUrl: '数据库 URL', saveDatabase: '保存数据库', databaseStored: '凭证已加密存储',
      oidc: '应用 OIDC', issuer: '签发者 URL', clientId: '客户端 ID', clientSecret: '客户端密钥（可选）', saveOidc: '保存 OIDC',
      members: '成员', subject: 'OIDC Subject', displayName: '显示名称', saveMember: '添加或更新成员',
      audit: '审计日志', refresh: '刷新', selectProject: '请选择项目', deployed: '已部署', notDeployed: '未部署', error: '请求失败', saved: '已保存',
      owner: '所有者', admin: '管理员', developer: '开发者', operator: '运维', auditor: '审计员'
    } }
  }
})

export default i18n
