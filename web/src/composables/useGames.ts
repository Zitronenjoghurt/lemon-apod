import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { GameSlug } from '@/api/types'
import { useStatus } from './useStatus'

export interface GameResult {
  /** `d:2026-08-08` for a daily, `f:1770000000000` for free play. Daily ids are what stop a day
   *  being recorded twice, and what the streak counts. */
  id: string
  /** The puzzle's day, on dailies only. */
  day?: string
  /** When it was played, in this browser's own time. */
  at: string
  /** The number worth comparing, best and averaged. Whatever the game's own scale is. */
  score: number
  /** How that score reads: `4,120 / 5,000`, `12 in a row`, `38 guesses`. */
  label: string
  /**
   * How each round went, as a band from 0 for the best to 4 for a miss.
   *
   * Kept as numbers rather than as the squares it shares, so the page can draw them as marks and
   * only the copied text has to reach for emoji.
   */
  bands?: Band[]
  /** Set only by the games that can be lost. */
  won?: boolean
}

/** 0 is as good as a round gets, 4 is a miss. What the steps in between mean is up to the game. */
export type Band = 0 | 1 | 2 | 3 | 4

const SHARE_BANDS: Record<GameSlug, string[]> = {
  date: ['\u{1F3AF}', '\u{1F7E9}', '\u{1F7E8}', '\u{1F7E7}', '\u{2B1B}'],
  words: ['\u{1F7E9}', '\u{1F7E9}', '\u{1F7E8}', '\u{1F7E7}', '\u{2B1B}'],
  order: ['\u{1F7E9}', '\u{1F7E9}', '\u{1F7E8}', '\u{1F7E7}', '\u{1F7E5}'],
  match: ['\u{1F7E9}', '\u{1F7E9}', '\u{1F7E8}', '\u{1F7E7}', '\u{1F7E5}'],
}

export type GameMode = 'daily' | 'free'

export interface GameStats {
  played: number
  best?: GameResult
  wins: number
  solvable: boolean
  streak: number
  longest: number
}

export const GAMES: { slug: GameSlug; name: string; path: string; icon: string; blurb: string }[] =
  [
    {
      slug: 'date',
      name: 'Guess the Date',
      path: '/games/date',
      icon: 'pi pi-calendar-clock',
      blurb:
        'A picture will come into focus step by step. Guess the date it appeared at as fast as possible',
    },
    {
      slug: 'words',
      name: 'Fill the Words',
      path: '/games/words',
      icon: 'pi pi-align-left',
      blurb:
        'An explanation with its words blacked out. Fill them in one at a time and clear the title to win.',
    },
    {
      slug: 'order',
      name: 'Which Came First',
      path: '/games/order',
      icon: 'pi pi-sort-alt',
      blurb:
        'Two pictures, months or years apart. Always guess which one appeared first to keep the run going.',
    },
    {
      slug: 'match',
      name: 'Match the Picture',
      path: '/games/match',
      icon: 'pi pi-images',
      blurb: 'One explanation and six pictures. Guess which picture it describes.',
    },
  ]

const KEEP = 250

export function gameKey(slug: GameSlug): string {
  return `apod:game:${slug}`
}

const stored = ref<Record<string, GameResult[]>>({})

function load(slug: GameSlug): GameResult[] {
  try {
    const raw = localStorage.getItem(gameKey(slug))
    const parsed: unknown = raw ? JSON.parse(raw) : []
    if (!Array.isArray(parsed)) return []
    return parsed.filter(
      (value): value is GameResult =>
        typeof value === 'object' &&
        value !== null &&
        typeof (value as GameResult).id === 'string' &&
        typeof (value as GameResult).score === 'number',
    )
  } catch {
    return []
  }
}

function results(slug: GameSlug): GameResult[] {
  stored.value[slug] ??= load(slug)
  return stored.value[slug]
}

export function hydrateGames(): void {
  stored.value = {}
  for (const game of GAMES) results(game.slug)
}

function today(): string {
  const now = new Date()
  const month = String(now.getMonth() + 1).padStart(2, '0')
  const day = String(now.getDate()).padStart(2, '0')
  return `${now.getFullYear()}-${month}-${day}`
}

export function useDailyDay() {
  const { publish } = useStatus()
  return computed(() => publish.value?.today ?? today())
}

export function useGameMode() {
  const route = useRoute()
  const router = useRouter()

  return computed<GameMode | null>({
    get: () => {
      const play = route.query.play
      return play === 'daily' || play === 'free' ? play : null
    },
    set: (next) => {
      void router.push({ path: route.path, query: next ? { play: next } : {} })
    },
  })
}

function shift(day: string, days: number): string {
  const at = new Date(`${day}T00:00:00`)
  at.setDate(at.getDate() + days)
  return `${at.getFullYear()}-${String(at.getMonth() + 1).padStart(2, '0')}-${String(at.getDate()).padStart(2, '0')}`
}

function streaks(days: Set<string>): { streak: number; longest: number } {
  if (!days.size) return { streak: 0, longest: 0 }

  const sorted = [...days].sort()
  let longest = 1
  let run = 1
  for (let index = 1; index < sorted.length; index++) {
    run = shift(sorted[index - 1], 1) === sorted[index] ? run + 1 : 1
    longest = Math.max(longest, run)
  }

  const now = today()
  let end = days.has(now) ? now : days.has(shift(now, -1)) ? shift(now, -1) : null
  let streak = 0
  while (end && days.has(end)) {
    streak += 1
    end = shift(end, -1)
  }

  return { streak, longest }
}

export function useProgress<T>(slug: GameSlug) {
  const key = `apod:game:${slug}:live`
  const KEEP_LIVE = 2
  const PUZZLE = /^(f|d:\d{4}-\d{2}-\d{2})$/

  function held(): Record<string, T> {
    try {
      const raw = localStorage.getItem(key)
      const parsed: unknown = raw ? JSON.parse(raw) : {}
      if (typeof parsed !== 'object' || parsed === null) return {}

      return Object.fromEntries(
        Object.entries(parsed as Record<string, T>).filter(([puzzle]) => PUZZLE.test(puzzle)),
      )
    } catch {
      return {}
    }
  }

  function write(all: Record<string, T>): void {
    try {
      localStorage.setItem(key, JSON.stringify(all))
    } catch {}
  }

  function save(puzzle: string, state: T): void {
    const all = held()
    delete all[puzzle]

    const entries = [...Object.entries(all), [puzzle, state] as const].slice(-KEEP_LIVE)
    write(Object.fromEntries(entries) as Record<string, T>)
  }

  function load(puzzle: string): T | undefined {
    return held()[puzzle]
  }

  function clear(puzzle: string): void {
    const all = held()
    if (!(puzzle in all)) return

    delete all[puzzle]
    write(all)
  }

  return { save, load, clear }
}

export function useGame(slug: GameSlug) {
  const history = computed(() => results(slug))

  function persist(): void {
    try {
      localStorage.setItem(gameKey(slug), JSON.stringify(results(slug).slice(0, KEEP)))
    } catch {}
  }

  /** The result already recorded for a day, if this browser has played it. */
  function resultFor(day: string | undefined): GameResult | undefined {
    if (!day) return undefined
    return history.value.find((result) => result.day === day)
  }

  function record(
    result: Omit<GameResult, 'id' | 'at'> & { day?: string },
  ): GameResult | undefined {
    const id = result.day ? `d:${result.day}` : `f:${Date.now()}`
    const kept = results(slug)
    if (kept.some((existing) => existing.id === id)) return undefined

    const full: GameResult = { ...result, id, at: new Date().toISOString() }
    stored.value[slug] = [full, ...kept].slice(0, KEEP)
    persist()
    return full
  }

  function clear(): void {
    stored.value[slug] = []
    persist()
  }

  const stats = computed(() => summarise(history.value))

  return { history, stats, record, resultFor, clear }
}

export function summarise(results: GameResult[]): GameStats {
  const days = new Set(results.filter((result) => result.day).map((result) => result.day as string))
  const { streak, longest } = streaks(days)

  return {
    played: results.length,
    best: results.reduce<GameResult | undefined>(
      (top, result) => (!top || result.score > top.score ? result : top),
      undefined,
    ),
    wins: results.filter((result) => result.won).length,
    solvable: results.some((result) => result.won !== undefined),
    streak,
    longest,
  }
}

export function useGameSummary() {
  const day = useDailyDay()

  return computed(() =>
    GAMES.map((game) => {
      const all = results(game.slug)
      const dailies = all.filter((result) => result.day)
      return {
        ...game,
        played: all.length,
        last: all[0],
        /** Today's daily, once it has been played, which is what the hub reports. */
        today: all.find((result) => result.day === day.value),
        streak: streaks(new Set(dailies.map((result) => result.day as string))).streak,
      }
    }),
  )
}

export function shareText(slug: GameSlug, result: GameResult, extra?: string): string {
  const game = GAMES.find((one) => one.slug === slug)
  const name = game?.name ?? slug
  const squares = SHARE_BANDS[slug]

  const lines = [result.day ? `APOD ${name}, ${result.day}` : `APOD ${name}`, result.label]
  if (result.bands?.length) {
    lines.push(result.bands.map((band) => squares[band] ?? squares[4]).join(''))
  }
  if (extra) lines.push(extra)
  lines.push(`${window.location.origin}${game?.path ?? ''}`)

  return lines.join('\n')
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    try {
      const area = document.createElement('textarea')
      area.value = text
      area.setAttribute('readonly', '')
      area.style.position = 'fixed'
      area.style.opacity = '0'
      document.body.appendChild(area)
      area.select()
      const copied = document.execCommand('copy')
      document.body.removeChild(area)
      return copied
    } catch {
      return false
    }
  }
}
