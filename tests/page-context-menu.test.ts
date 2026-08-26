import { buildPageContextMenu, pageSaveFilename } from '../src/main/page-context-menu'

const actions = {
  back: vi.fn(),
  forward: vi.fn(),
  reload: vi.fn(),
  savePage: vi.fn(),
  printPage: vi.fn(),
  viewSource: vi.fn(),
  inspect: vi.fn()
}

it('builds localized page commands with navigation availability', () => {
  const english = buildPageContextMenu('en', {
    canGoBack: false,
    canGoForward: true,
    canSave: true,
    canPrint: true,
    canViewSource: true
  }, actions)
  const chinese = buildPageContextMenu('zh-CN', {
    canGoBack: true,
    canGoForward: false,
    canSave: false,
    canPrint: false,
    canViewSource: false
  }, actions)

  expect(english.map((item) => item.label ?? item.type)).toEqual([
    'Back', 'Forward', 'Reload', 'separator', 'Save page as...', 'Print', 'View page source', 'separator', 'Inspect'
  ])
  expect(english[0].enabled).toBe(false)
  expect(english[1].enabled).toBe(true)
  expect(chinese.map((item) => item.label ?? item.type)).toEqual([
    '后退', '前进', '重新加载', 'separator', '网页另存为...', '打印', '查看网页源代码', 'separator', '检查'
  ])
  expect(chinese[4].enabled).toBe(false)
  expect(chinese[5].enabled).toBe(false)
  expect(chinese[6].enabled).toBe(false)
  expect(english[2].click).toBe(actions.reload)
  expect(english[5].click).toBe(actions.printPage)
})

it('creates safe HTML filenames from titles and URLs', () => {
  expect(pageSaveFilename('Example: Home / News?', 'https://example.com')).toBe('Example_ Home _ News_.html')
  expect(pageSaveFilename('', 'https://www.example.com/path')).toBe('www.example.com.html')
  expect(pageSaveFilename('report.html', 'https://example.com')).toBe('report.html')
  expect(pageSaveFilename('   ', 'not a url')).toBe('page.html')
})
