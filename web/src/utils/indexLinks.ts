import type { Contributor, ObjectMention } from '@/api/types'

export const INDEX_LINK_CLASS = 'index-link'

const MARK =
  '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" ' +
  'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
  '<path d="m22 11-1.296-1.296a2.4 2.4 0 0 0-3.408 0L11 16"/><path d="M4 8a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2"/><circle cx="13" cy="7" r="1" fill="currentColor"/><rect x="8" y="2" width="14" height="14" rx="2"/>' +
  '</svg>'

export interface IndexTarget {
  patterns: RegExp[]
  href: string
  title: string
}

const LETTER = '[\\p{L}\\p{N}]'
const BOUNDED = (body: string) => new RegExp(`(?<!${LETTER})(?:${body})(?!${LETTER})`, 'iu')

function quote(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function phrase(text: string): RegExp {
  return BOUNDED(quote(text.trim()).replace(/\s+/g, '\\s+'))
}

const SPELLINGS: Record<string, string> = {
  m: 'M|Messier',
  sh2: 'Sh\\s?2',
  ced: 'Ced|Cederblad',
}

function designation(id: string): RegExp | null {
  const catalogued = /^([A-Za-z]+)\s?-?(\d+)$/.exec(id)
  if (catalogued) {
    const [, prefix, number] = catalogued as unknown as [string, string, string]
    const spelled = SPELLINGS[prefix.toLowerCase()] ?? quote(prefix)
    return BOUNDED(`(?:${spelled})[\\s-]?${number}`)
  }

  const supernova = /^SN\s?(\d{4}[A-Za-z]{0,2})$/i.exec(id)
  if (supernova) return BOUNDED(`SN\\s?${supernova[1]}`)

  return null
}

export function objectTargets(mentions: ObjectMention[]): IndexTarget[] {
  return mentions.map((mention) => ({
    patterns: [phrase(mention.name), designation(mention.id)].filter(
      (pattern): pattern is RegExp => pattern !== null,
    ),
    href: `/objects/${encodeURIComponent(mention.id)}`,
    title: `${mention.id} in the archive: ${mention.entries.toLocaleString()} entries`,
  }))
}

export function contributorTargets(contributors: Contributor[]): IndexTarget[] {
  return contributors
    .filter((contributor) => contributor.entries > 1)
    .map((contributor) => ({
      patterns: [phrase(contributor.label)],
      href: `/credits/${encodeURIComponent(contributor.id)}`,
      title: `${contributor.label} in the archive: ${contributor.entries.toLocaleString()} entries`,
    }))
}

export function withIndexLinks(
  html: string,
  targets: IndexTarget[],
  placed: Set<string> = new Set(),
): string {
  if (!targets.length || !html) return html

  const template = document.createElement('template')
  template.innerHTML = html

  for (const target of targets) {
    if (placed.has(target.href)) continue
    if (place(template.content, target)) placed.add(target.href)
  }

  return template.innerHTML
}

function place(root: DocumentFragment, target: IndexTarget): boolean {
  for (const pattern of target.patterns) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
    let node: Node | null

    while ((node = walker.nextNode())) {
      const text = node as Text
      if (text.parentElement?.closest(`.${INDEX_LINK_CLASS}`)) continue

      const match = pattern.exec(text.data)
      if (!match) continue

      text.splitText(match.index + match[0].length)
      const host: ChildNode = text.parentElement?.closest('a') ?? text
      host.after(mark(target))
      return true
    }
  }

  return false
}

function mark(target: IndexTarget): HTMLAnchorElement {
  const anchor = document.createElement('a')
  anchor.className = INDEX_LINK_CLASS
  anchor.setAttribute('href', target.href)
  anchor.title = target.title
  anchor.setAttribute('aria-label', target.title)
  anchor.innerHTML = MARK
  return anchor
}
