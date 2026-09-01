const FULL = new Intl.DateTimeFormat(undefined, {
  year: 'numeric',
  month: 'long',
  day: 'numeric',
})

export function formatDate(date: string): string {
  const parsed = parse(date)
  return parsed ? FULL.format(parsed) : date
}

export function previousDay(date: string): string | null {
  return shift(date, -1)
}

export function nextDay(date: string): string | null {
  return shift(date, 1)
}

export function monthDay(date: string): string {
  return date.slice(5)
}

export function year(date: string): number {
  return Number.parseInt(date.slice(0, 4), 10)
}

export function month(date: string): number {
  return Number.parseInt(date.slice(5, 7), 10)
}

export function archivePath(date: string): string {
  return `/archive/${date.slice(0, 4)}/${date.slice(5, 7)}`
}

const MONTH_YEAR = new Intl.DateTimeFormat(undefined, { year: 'numeric', month: 'long' })

export function formatMonth(date: string): string {
  const parsed = parse(date)
  return parsed ? MONTH_YEAR.format(parsed) : date.slice(0, 7)
}

export const FIRST_ENTRY = '1995-06-16'

export function daysBetween(from: string, to: string): number {
  const one = parse(from)
  const other = parse(to)
  if (!one || !other) return 0
  return Math.round(Math.abs(one.getTime() - other.getTime()) / 86_400_000)
}

export function describeGap(days: number): string {
  if (days === 0) return 'exactly right'
  if (days === 1) return 'one day out'
  if (days < 31) return `${days} days out`

  const months = Math.round(days / 30.44)
  if (days < 365) return `${months} month${months === 1 ? '' : 's'} out`

  const years = days / 365.25
  const rounded = years < 10 ? Math.round(years * 10) / 10 : Math.round(years)
  return `${rounded} year${rounded === 1 ? '' : 's'} out`
}

export function clampDate(date: string, first: string, last: string): string {
  if (date < first) return first
  if (date > last) return last
  return date
}

function shift(date: string, days: number): string | null {
  const parsed = parse(date)
  if (!parsed) return null

  parsed.setUTCDate(parsed.getUTCDate() + days)
  return parsed.toISOString().slice(0, 10)
}

function parse(date: string): Date | null {
  const parsed = new Date(`${date}T00:00:00Z`)
  return Number.isNaN(parsed.getTime()) ? null : parsed
}

export function isoDate(text: string): string | null {
  const trimmed = text.trim()
  if (!/^\d{4}-\d{2}-\d{2}$/.test(trimmed)) return null

  const parsed = parse(trimmed)
  return parsed?.toISOString().slice(0, 10) === trimmed ? trimmed : null
}
