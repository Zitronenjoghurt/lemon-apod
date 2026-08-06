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

export const FIRST_ENTRY = '1995-06-16'

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
