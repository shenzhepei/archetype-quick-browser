import { defineConfig } from 'vitepress'

const guide = [
  { text: 'Browser', link: '/guide/browser' },
  { text: 'Cloud functions', link: '/guide/functions' },
  { text: 'Data and jobs', link: '/guide/data-jobs' },
  { text: 'Self-hosting', link: '/guide/self-hosting' }
]

const zhGuide = [
  { text: '浏览器', link: '/zh-CN/guide/browser' },
  { text: '云函数', link: '/zh-CN/guide/functions' },
  { text: '数据与队列', link: '/zh-CN/guide/data-jobs' },
  { text: '自托管', link: '/zh-CN/guide/self-hosting' }
]

export default defineConfig({
  title: 'Archetype Runtime',
  description: 'Browser-native capabilities backed by self-hosted functions, data, and durable jobs.',
  base: process.env.DOCS_BASE ?? '/archetype-runtime-browser/',
  cleanUrls: true,
  lastUpdated: true,
  themeConfig: {
    logo: '/favicon.png',
    nav: [
      { text: 'Guide', link: '/guide/browser' },
      { text: 'API', link: '/reference/api' },
      { text: 'Security', link: '/security' },
      { text: '简体中文', link: '/zh-CN/' }
    ],
    sidebar: [
      { text: 'Guide', items: guide },
      { text: 'Reference', items: [{ text: 'Web API', link: '/reference/api' }, { text: 'CLI', link: '/reference/cli' }] },
      { text: 'Security', link: '/security' }
    ],
    socialLinks: [{ icon: 'github', link: 'https://github.com/shenzhepei/archetype-runtime-browser' }],
    search: { provider: 'local' },
    footer: { message: 'Released under Apache-2.0.', copyright: 'Copyright shenzhepei' }
  },
  locales: {
    root: { label: 'English', lang: 'en' },
    'zh-CN': {
      label: '简体中文',
      lang: 'zh-CN',
      title: 'Archetype Runtime',
      description: '由自托管云函数、数据与可靠队列支持的浏览器原生能力。',
      themeConfig: {
        nav: [
          { text: '指南', link: '/zh-CN/guide/browser' },
          { text: 'API', link: '/zh-CN/reference/api' },
          { text: '安全', link: '/zh-CN/security' },
          { text: 'English', link: '/' }
        ],
        sidebar: [
          { text: '指南', items: zhGuide },
          { text: '参考', items: [{ text: 'Web API', link: '/zh-CN/reference/api' }, { text: 'CLI', link: '/zh-CN/reference/cli' }] },
          { text: '安全', link: '/zh-CN/security' }
        ],
        outline: { label: '本页目录' },
        docFooter: { prev: '上一页', next: '下一页' },
        lastUpdated: { text: '最后更新' }
      }
    }
  }
})
