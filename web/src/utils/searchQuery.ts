export interface QueryTerm {
  words: string[]
  prefix: boolean
}

export function queryTerms(raw: string): QueryTerm[] {
  const tokens = tokenize(raw)

  return tokens
    .filter((token) => !token.negated && token.kind === 'term')
    .map((token) => ({ words: words(token.text), prefix: token.prefix }))
    .filter((term) => term.words.length > 0)
}

interface Token {
  kind: 'term' | 'or'
  text: string
  quoted: boolean
  prefix: boolean
  negated: boolean
}

function tokenize(raw: string): Token[] {
  const chars = [...raw]
  const tokens: Token[] = []
  let at = 0
  let negateNext = false

  while (at < chars.length) {
    if (isSpace(chars[at]!)) {
      at += 1
      continue
    }

    let negated = negateNext
    negateNext = false

    if (chars[at] === '-' && at + 1 < chars.length && !isSpace(chars[at + 1]!)) {
      negated = true
      at += 1
    }

    const quoted = chars[at] === '"'
    let text = ''

    if (quoted) {
      at += 1
      while (at < chars.length && chars[at] !== '"') text += chars[at++]
      if (at < chars.length) at += 1
    } else {
      while (at < chars.length && !isSpace(chars[at]!)) text += chars[at++]
    }

    let prefix = false
    if (quoted) {
      if (chars[at] === '*') {
        prefix = true
        at += 1
      }
    } else if (text.endsWith('*')) {
      prefix = true
      text = text.replace(/\*+$/, '')
    }

    if (!quoted && !negated) {
      if (text === 'OR') {
        tokens.push({ kind: 'or', text, quoted, prefix: false, negated })
        continue
      }
      if (text === 'AND') continue
      if (text === 'NOT') {
        negateNext = true
        continue
      }
    }

    tokens.push({ kind: 'term', text, quoted, prefix, negated })
  }

  const last = tokens.at(-1)
  if (last?.kind === 'term' && !last.quoted && !last.negated) last.prefix = true

  return tokens
}

function isSpace(char: string): boolean {
  return /\s/.test(char)
}

/** How FTS5's tokenizer splits a term: on anything that is not a letter or a digit. */
function words(text: string): string[] {
  return text.split(/[^\p{L}\p{N}]+/u).filter(Boolean)
}
