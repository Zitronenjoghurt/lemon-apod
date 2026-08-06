import { FIRST_ENTRY } from './date'

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
