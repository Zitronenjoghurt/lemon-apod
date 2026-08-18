import { EXTERNAL_WARNING_KEY, hydrateExternalLinks } from './useExternalLinks'
import { hydrateFavorites } from './useFavorites'
import { gameKey, GAMES, hydrateGames } from './useGames'
import { ARCHIVE_VIEW_KEY, hydratePreferences, WEEK_START_KEY } from './usePreferences'
import { filterKey, hydrateRead, READ_SCOPES } from './useRead'
import { hydrateTheme } from './useTheme'
import { CARD_KEY, hydrateRatingCard } from './useRating'
import { hydrateWelcome, WELCOME_KEY } from './useWelcome'

const APP = 'lemon-apod'
const FORMAT = 1

type Shape = 'dates' | 'scalar' | 'results'

interface Field {
  key: string
  shape: Shape
  label: string
  hydrate: () => void
}

const FIELDS: Field[] = [
  { key: 'apod:favorites', shape: 'dates', label: 'favorites', hydrate: hydrateFavorites },
  { key: 'apod:read', shape: 'dates', label: 'read entries', hydrate: hydrateRead },
  ...READ_SCOPES.map((scope) => ({
    key: filterKey(scope),
    shape: 'scalar' as Shape,
    label: `${scope} read filter`,
    hydrate: hydrateRead,
  })),
  { key: 'apod:theme', shape: 'scalar', label: 'theme', hydrate: hydrateTheme },
  { key: WEEK_START_KEY, shape: 'scalar', label: 'week start', hydrate: hydratePreferences },
  { key: ARCHIVE_VIEW_KEY, shape: 'scalar', label: 'archive layout', hydrate: hydratePreferences },
  {
    key: EXTERNAL_WARNING_KEY,
    shape: 'scalar',
    label: 'link warning',
    hydrate: hydrateExternalLinks,
  },
  { key: WELCOME_KEY, shape: 'scalar', label: 'welcome note', hydrate: hydrateWelcome },
  { key: CARD_KEY, shape: 'scalar', label: 'rating card', hydrate: hydrateRatingCard },
  ...GAMES.map((game) => ({
    key: gameKey(game.slug),
    shape: 'results' as Shape,
    label: `${game.name} results`,
    hydrate: hydrateGames,
  })),
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

function readResults(raw: string | null): { id: string; at: string }[] {
  if (!raw) return []
  try {
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed.filter(
      (value): value is { id: string; at: string } =>
        typeof value === 'object' &&
        value !== null &&
        typeof (value as { id?: unknown }).id === 'string' &&
        typeof (value as { at?: unknown }).at === 'string',
    )
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
      } else if (field.shape === 'results') {
        const existing = mode === 'merge' ? readResults(localStorage.getItem(field.key)) : []
        const merged = [...existing]
        const known = new Set(existing.map((result) => result.id))
        for (const result of readResults(incoming)) {
          if (!known.has(result.id)) {
            known.add(result.id)
            merged.push(result)
          }
        }
        merged.sort((one, other) => other.at.localeCompare(one.at))

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
