import type { MenuItemConstructorOptions } from 'electron'

export type PageMenuLanguage = 'en' | 'zh-CN'

export interface PageMenuState {
  canGoBack: boolean
  canGoForward: boolean
  canSave: boolean
  canPrint: boolean
  canViewSource: boolean
}

export interface PageMenuActions {
  back(): void
  forward(): void
  reload(): void
  savePage(): void
  printPage(): void
  viewSource(): void
  inspect(): void
}

export const pageMenuLabels = {
  en: {
    back: 'Back',
    forward: 'Forward',
    reload: 'Reload',
    savePageAs: 'Save page as...',
    print: 'Print',
    viewSource: 'View page source',
    inspect: 'Inspect',
    htmlFile: 'HTML document',
    saveFailed: 'Could not save page',
    saveFailedDetail: 'Archetype could not save this page as HTML.',
    printFailed: 'Could not print page',
    printFailedDetail: 'Archetype could not open the print dialog for this page.',
    noPrinters: 'No system printer is configured.'
  },
  'zh-CN': {
    back: '后退',
    forward: '前进',
    reload: '重新加载',
    savePageAs: '网页另存为...',
    print: '打印',
    viewSource: '查看网页源代码',
    inspect: '检查',
    htmlFile: 'HTML 文档',
    saveFailed: '无法保存网页',
    saveFailedDetail: 'Archetype 无法将此网页保存为 HTML。',
    printFailed: '无法打印网页',
    printFailedDetail: 'Archetype 无法为此网页打开打印对话框。',
    noPrinters: '系统尚未配置打印机。'
  }
} as const

export function buildPageContextMenu(
  language: PageMenuLanguage,
  state: PageMenuState,
  actions: PageMenuActions
): MenuItemConstructorOptions[] {
  const labels = pageMenuLabels[language]
  return [
    { label: labels.back, enabled: state.canGoBack, click: actions.back },
    { label: labels.forward, enabled: state.canGoForward, click: actions.forward },
    { label: labels.reload, click: actions.reload },
    { type: 'separator' },
    { label: labels.savePageAs, enabled: state.canSave, click: actions.savePage },
    { label: labels.print, enabled: state.canPrint, click: actions.printPage },
    { label: labels.viewSource, enabled: state.canViewSource, click: actions.viewSource },
    { type: 'separator' },
    { label: labels.inspect, click: actions.inspect }
  ]
}

export function pageSaveFilename(title: string, url: string): string {
  let candidate = title.trim()
  if (!candidate) {
    try {
      candidate = new URL(url).hostname
    } catch {
      candidate = ''
    }
  }
  const cleaned = candidate
    .replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_')
    .replace(/[. ]+$/g, '')
    .trim()
    .replace(/\.html?$/i, '')
  const stem = cleaned.slice(0, 115).replace(/[. ]+$/g, '') || 'page'
  return `${stem}.html`
}
