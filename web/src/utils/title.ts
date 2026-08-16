export const SITE = 'APOD Archive'

const MONTHS = [
  'January',
  'February',
  'March',
  'April',
  'May',
  'June',
  'July',
  'August',
  'September',
  'October',
  'November',
  'December',
]

export function pageTitle(name: string): string {
  return `${name} · ${SITE}`
}

export function entryTitle(title: string, date: string): string {
  return `${title} (APOD ${date})`
}

export function pictureTitle(title: string, appearances: number): string {
  return `${title} · Shown ${appearances}×`
}

export function archiveTitle(year?: string, month?: string): string {
  if (!year) return pageTitle('Archive')

  const index = month ? Number.parseInt(month, 10) - 1 : -1
  const named = MONTHS[index]

  return pageTitle(named ? `APOD in ${named} ${year}` : `APOD in ${year}`)
}

export function setTitle(title: string): void {
  document.title = title
}
