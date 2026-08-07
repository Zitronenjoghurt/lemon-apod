import type { QueryTerm } from './searchQuery'

export const HIT_CLASS = 'search-hit'

export interface Highlighted {
  html: string
  count: number
}

export function highlightHtml(html: string, terms: QueryTerm[]): Highlighted {
  const pattern = build(terms)
  if (!pattern) return { html, count: 0 }

  const template = document.createElement('template')
  template.innerHTML = html

  const count = paint(template.content, pattern)
  return { html: template.innerHTML, count }
}

export function highlightText(text: string, terms: QueryTerm[]): Highlighted {
  const pattern = build(terms)
  if (!pattern) return { html: escape(text), count: 0 }

  const template = document.createElement('template')
  template.append(document.createTextNode(text))

  const count = paint(template.content, pattern)
  return { html: template.innerHTML, count }
}

function paint(root: DocumentFragment, pattern: RegExp): number {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  const targets: Text[] = []
  let node: Node | null

  while ((node = walker.nextNode())) {
    if (node.textContent?.trim()) targets.push(node as Text)
  }

  let count = 0
  for (const target of targets) count += paintNode(target, pattern)
  return count
}

function paintNode(node: Text, pattern: RegExp): number {
  const text = node.data
  const { folded, origin } = fold(text)

  pattern.lastIndex = 0
  const pieces = document.createDocumentFragment()
  let consumed = 0
  let count = 0
  let match: RegExpExecArray | null

  while ((match = pattern.exec(folded))) {
    if (match[0].length === 0) {
      pattern.lastIndex += 1
      continue
    }

    const start = origin[match.index] ?? text.length
    const end = (origin[match.index + match[0].length - 1] ?? text.length - 1) + 1

    if (start > consumed) pieces.append(text.slice(consumed, start))

    const mark = document.createElement('mark')
    mark.className = HIT_CLASS
    mark.append(text.slice(start, end))
    pieces.append(mark)

    consumed = end
    count += 1
  }

  if (count === 0) return 0

  if (consumed < text.length) pieces.append(text.slice(consumed))
  node.replaceWith(pieces)
  return count
}

function fold(text: string): { folded: string; origin: number[] } {
  let folded = ''
  const origin: number[] = []

  for (let index = 0; index < text.length; index += 1) {
    const stripped = text[index]!.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase()

    folded += stripped
    for (let n = 0; n < stripped.length; n += 1) origin.push(index)
  }

  return { folded, origin }
}

function build(terms: QueryTerm[]): RegExp | null {
  const sorted = [...terms].sort((a, b) => b.words.join(' ').length - a.words.join(' ').length)

  const parts = sorted
    .map((term) => {
      const words = term.words.map(quote)
      if (words.length === 0) return null

      // FTS5 phrases are adjacent tokens, so anything non-alphanumeric may sit between them.
      const body = words.join(BETWEEN)
      const tail = term.prefix ? `${LETTER}*` : ''
      return `(?<!${LETTER})${body}${tail}(?!${LETTER})`
    })
    .filter((part): part is string => part !== null)

  return parts.length ? new RegExp(parts.join('|'), 'gu') : null
}

const LETTER = '[\\p{L}\\p{N}]'
const BETWEEN = '[^\\p{L}\\p{N}]+'

function quote(word: string): string {
  return word
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .toLowerCase()
    .replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function escape(text: string): string {
  const node = document.createElement('span')
  node.textContent = text
  return node.innerHTML
}
