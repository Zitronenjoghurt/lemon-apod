export type ChangeKind = 'same' | 'added' | 'removed'

export interface Change {
  kind: ChangeKind
  text: string
}

export function diffWords(before: string, after: string): Change[] {
  return merge(script(tokenize(before), tokenize(after)))
}

export function countWords(changes: Change[], kind: ChangeKind): number {
  return changes
    .filter((change) => change.kind === kind)
    .reduce((total, change) => total + words(change.text), 0)
}

export function words(text: string): number {
  return text.split(/\s+/).filter(Boolean).length
}

const TOO_FAR = 1500

function tokenize(text: string): string[] {
  return text.match(/\S+\s*/g) ?? []
}

function script(before: string[], after: string[]): Change[] {
  let head = 0
  while (head < before.length && head < after.length && before[head] === after[head]) head++

  let tail = 0
  while (
    tail < before.length - head &&
    tail < after.length - head &&
    before[before.length - 1 - tail] === after[after.length - 1 - tail]
  ) {
    tail++
  }

  const a = before.slice(head, before.length - tail)
  const b = after.slice(head, after.length - tail)

  const middle =
    a.length > TOO_FAR && b.length > TOO_FAR
      ? [
          { kind: 'removed' as const, text: a.join('') },
          { kind: 'added' as const, text: b.join('') },
        ]
      : myers(a, b)

  return [
    ...before.slice(0, head).map((text) => ({ kind: 'same' as const, text })),
    ...middle,
    ...before.slice(before.length - tail).map((text) => ({ kind: 'same' as const, text })),
  ]
}

function myers(a: string[], b: string[]): Change[] {
  if (!a.length) return b.map((text) => ({ kind: 'added', text }))
  if (!b.length) return a.map((text) => ({ kind: 'removed', text }))

  const max = a.length + b.length
  const ends = new Array<number>(2 * max + 1).fill(0)
  const trace: number[][] = []

  for (let d = 0; d <= max; d++) {
    trace.push(ends.slice())

    for (let k = -d; k <= d; k += 2) {
      const down = k === -d || (k !== d && ends[max + k - 1] < ends[max + k + 1])
      let x = down ? ends[max + k + 1] : ends[max + k - 1] + 1
      let y = x - k

      while (x < a.length && y < b.length && a[x] === b[y]) {
        x++
        y++
      }

      ends[max + k] = x
      if (x >= a.length && y >= b.length) return backtrack(a, b, trace)
    }
  }

  return backtrack(a, b, trace)
}

function backtrack(a: string[], b: string[], trace: number[][]): Change[] {
  const max = a.length + b.length
  const out: Change[] = []

  let x = a.length
  let y = b.length

  for (let d = trace.length - 1; d >= 0; d--) {
    const ends = trace[d]
    const k = x - y

    const down = k === -d || (k !== d && ends[max + k - 1] < ends[max + k + 1])
    const previous = down ? k + 1 : k - 1
    const fromX = ends[max + previous]
    const fromY = fromX - previous

    while (x > fromX && y > fromY) {
      out.push({ kind: 'same', text: a[--x] })
      y--
    }

    if (d === 0) break
    if (down) out.push({ kind: 'added', text: b[--y] })
    else out.push({ kind: 'removed', text: a[--x] })
  }

  return out.reverse()
}

function merge(changes: Change[]): Change[] {
  const out: Change[] = []

  for (const change of changes) {
    const last = out[out.length - 1]
    if (last && last.kind === change.kind) last.text += change.text
    else out.push({ ...change })
  }

  return out.filter((change) => change.text.length > 0)
}
