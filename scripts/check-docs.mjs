import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { dirname, extname, join, normalize, relative, resolve, sep } from 'node:path'

const docsRoot = resolve('apps/docs')
const publicRoot = join(docsRoot, 'public')
const zhRoot = join(docsRoot, 'zh-CN')

function markdownFiles(root) {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const path = join(root, entry.name)
    if (entry.isDirectory()) {
      return entry.name === '.vitepress' || entry.name === 'node_modules' ? [] : markdownFiles(path)
    }
    return entry.name.endsWith('.md') ? [path] : []
  })
}

const allFiles = markdownFiles(docsRoot)
const englishFiles = allFiles.filter((file) => !file.startsWith(`${zhRoot}${sep}`))
const chineseFiles = allFiles.filter((file) => file.startsWith(`${zhRoot}${sep}`))
const errors = []

const englishPages = new Set(englishFiles.map((file) => relative(docsRoot, file)))
const chinesePages = new Set(chineseFiles.map((file) => relative(zhRoot, file)))

for (const page of englishPages) {
  if (!chinesePages.has(page)) errors.push(`Missing Simplified Chinese page: zh-CN/${page}`)
}
for (const page of chinesePages) {
  if (!englishPages.has(page)) errors.push(`Missing English page: ${page}`)
}

function routeTarget(target) {
  const cleanTarget = decodeURIComponent(target.split(/[?#]/, 1)[0])
  if (!cleanTarget) return null
  if (/^(?:[a-z]+:|\/\/)/i.test(cleanTarget)) return null

  if (cleanTarget.startsWith('/')) {
    const path = cleanTarget.slice(1)
    if (extname(path)) return join(publicRoot, path)
    return join(docsRoot, path === '' ? 'index.md' : `${path}.md`)
  }

  const path = resolve(dirname(currentFile), cleanTarget)
  if (extname(path)) return path
  return `${path}.md`
}

let currentFile = ''
for (const file of allFiles) {
  currentFile = file
  const source = readFileSync(file, 'utf8')
  const links = source.matchAll(/!?(?:\[[^\]]*\])\(([^)\s]+)(?:\s+['"][^'"]*['"])?\)/g)
  for (const match of links) {
    const target = routeTarget(match[1])
    if (target && !existsSync(normalize(target))) {
      errors.push(`${relative(docsRoot, file)} links to missing ${match[1]}`)
    }
  }
}

if (errors.length > 0) {
  console.error(errors.join('\n'))
  process.exitCode = 1
} else {
  console.log(`Validated ${englishPages.size} English/Chinese page pairs and local documentation links.`)
}
