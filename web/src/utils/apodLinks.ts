import { FIRST_ENTRY } from './date'
import { APOD_URL } from './links'
import type { ApodEntry } from '@/api/types'

const DECOMMISSIONED = ['apod.nasa.gov', 'www.apod.nasa.gov', 'antwrp.gsfc.nasa.gov']

const HREF =
  /href="https?:\/\/(?:www\.)?(?:apod\.nasa\.gov|antwrp\.gsfc\.nasa\.gov)\/apod\/ap(\d{6})\.html"/gi
const INTERNAL_ATTRIBUTE = 'data-apod-entry'

export function withInternalLinks(html: string): string {
  return html.replace(HREF, (original, stamp: string) => {
    const date = fromStamp(stamp)
    return date ? `${INTERNAL_ATTRIBUTE} href="/${date}"` : original
  })
}

function fromStamp(stamp: string): string | null {
  const short = Number.parseInt(stamp.slice(0, 2), 10)
  const month = Number.parseInt(stamp.slice(2, 4), 10)
  const day = Number.parseInt(stamp.slice(4, 6), 10)
  const year = short >= 95 ? 1900 + short : 2000 + short

  const date = new Date(Date.UTC(year, month - 1, day))
  if (
    date.getUTCFullYear() !== year ||
    date.getUTCMonth() !== month - 1 ||
    date.getUTCDate() !== day
  ) {
    return null
  }

  const iso = date.toISOString().slice(0, 10)
  return iso >= FIRST_ENTRY ? iso : null
}

export function officialUrl(entry: Pick<ApodEntry, 'source_url'>): string | null {
  const host = entry.source_url.split('//')[1] ?? entry.source_url
  return DECOMMISSIONED.some((dead) => host.startsWith(dead)) ? null : entry.source_url
}

export function originalPath(date: string): string {
  return `/${date}/original`
}

export function apodPageUrl(entry: Pick<ApodEntry, 'source_url'>): string {
  return officialUrl(entry) ?? APOD_URL
}
