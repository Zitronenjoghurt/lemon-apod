import { EXTERNAL_WARNING_KEY, hydrateExternalLinks } from './useExternalLinks'
import { hydrateFavorites } from './useFavorites'
import { ARCHIVE_VIEW_KEY, hydratePreferences, WEEK_START_KEY } from './usePreferences'
import { hydrateRead } from './useRead'
import { hydrateTheme } from './useTheme'

const APP = 'lemon-apod'
const FORMAT = 1

type Shape = 'dates' | 'scalar'

interface Field {
  key: string
  shape: Shape
  label: string
  hydrate: () => void
}

const FIELDS: Field[] = [
  { key: 'apod:favorites', shape: 'dates', label: 'favorites', hydrate: hydrateFavorites },
  { key: 'apod:read', shape: 'dates', label: 'read entries', hydrate: hydrateRead },
  { key: 'apod:read-filter', shape: 'scalar', label: 'read filter', hydrate: hydrateRead },
  { key: 'apod:theme', shape: 'scalar', label: 'theme', hydrate: hydrateTheme },
  { key: WEEK_START_KEY, shape: 'scalar', label: 'week start', hydrate: hydratePreferences },
  { key: ARCHIVE_VIEW_KEY, shape: 'scalar', label: 'archive layout', hydrate: hydratePreferences },
  {
    key: EXTERNAL_WARNING_KEY,
    shape: 'scalar',
    label: 'link warning',
    hydrate: hydrateExternalLinks,
  },
]

export type ImportMode = 'merge' | 'replace'

export interface ImportSummary {
  changes: string[]
  total: number
}

export class BackupError extends Error {}

interface Backup {
  app: string
  version: number
  exported_at: string
  data: Record<string, string>
}

function readDates(raw: string | null): string[] {
  if (!raw) return []
  try {
    const parsed: unknown = JSON.parse(raw)
    return Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === 'string') : []
  } catch {
    return []
  }
}

function stamp(): string {
  const now = new Date()
  const month = String(now.getMonth() + 1).padStart(2, '0')
  const day = String(now.getDate()).padStart(2, '0')
  return `${now.getFullYear()}-${month}-${day}`
}

export function useSiteData() {
  function snapshot(): Backup {
    const data: Record<string, string> = {}
    for (const field of FIELDS) {
      const raw = localStorage.getItem(field.key)
      if (raw !== null) data[field.key] = raw
    }

    return { app: APP, version: FORMAT, exported_at: new Date().toISOString(), data }
  }

  function download(): string {
    const name = `apod-archive-${stamp()}.json`
    const blob = new Blob([JSON.stringify(snapshot(), null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)

    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = name
    anchor.click()

    setTimeout(() => URL.revokeObjectURL(url), 1_000)
    return name
  }

  function parse(text: string): Backup {
    let parsed: unknown
    try {
      parsed = JSON.parse(text)
    } catch {
      throw new BackupError('That file is not JSON.')
    }

    if (typeof parsed !== 'object' || parsed === null) {
      throw new BackupError('That file does not look like a backup.')
    }

    const backup = parsed as Partial<Backup>
    if (backup.app !== APP) {
      throw new BackupError('That backup was written by a different site.')
    }
    if (backup.version !== FORMAT) {
      throw new BackupError(
        `That backup is format ${String(backup.version)}, and this build reads format ${FORMAT}.`,
      )
    }
    if (typeof backup.data !== 'object' || backup.data === null) {
      throw new BackupError('That backup has nothing in it.')
    }

    const known = new Set(FIELDS.map((field) => field.key))
    const data: Record<string, string> = {}
    for (const [key, value] of Object.entries(backup.data)) {
      if (!known.has(key)) continue
      if (typeof value !== 'string') {
        throw new BackupError(`The ${key} entry in that backup is the wrong shape.`)
      }
      data[key] = value
    }

    if (!Object.keys(data).length) {
      throw new BackupError('That backup holds nothing this build knows how to read.')
    }

    return { ...(backup as Backup), data }
  }

  function apply(backup: Backup, mode: ImportMode): ImportSummary {
    const changes: string[] = []
    let total = 0

    for (const field of FIELDS) {
      const incoming = backup.data[field.key]

      if (incoming === undefined) {
        if (mode === 'replace' && localStorage.getItem(field.key) !== null) {
          localStorage.removeItem(field.key)
          changes.push(`${field.label} cleared`)
        }
        continue
      }

      if (field.shape === 'dates') {
        const existing = mode === 'merge' ? readDates(localStorage.getItem(field.key)) : []
        const merged = [...new Set([...existing, ...readDates(incoming)])].sort()

        localStorage.setItem(field.key, JSON.stringify(merged))
        const added = merged.length - existing.length
        total += merged.length
        changes.push(
          mode === 'merge'
            ? `${added.toLocaleString()} new ${field.label}`
            : `${merged.length.toLocaleString()} ${field.label}`,
        )
      } else {
        localStorage.setItem(field.key, incoming)
      }
    }

    for (const field of FIELDS) field.hydrate()

    return { changes, total }
  }

  return {
    download,
    restore: (text: string, mode: ImportMode): ImportSummary => apply(parse(text), mode),
  }
}
