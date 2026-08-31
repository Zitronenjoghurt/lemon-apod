import { ref } from 'vue'
import type {
  ApodEntry,
  ApodSummary,
  Ballot,
  Board,
  Cast,
  Coverage,
  FieldDivergence,
  Forgotten,
  GamePicture,
  Gap,
  HostCount,
  KindFilter,
  KnownWord,
  Listing,
  MatchAnswer,
  MatchRound,
  Migration,
  OrderPair,
  Page,
  Picture,
  PictureAppearances,
  PictureSort,
  Puzzle,
  RatingCategory,
  RatingOutcome,
  RatingTerms,
  Resource,
  ResourceRefs,
  ResourceSort,
  Reveal,
  SearchResults,
  Sky,
  SortOrder,
  Stats,
  Status,
  Timeline,
  WeatherReport,
  Word,
  WordSort,
  WordsRound,
  WordUse,
} from './types'

export const throttled = ref(false)

const MAX_ATTEMPTS = 5
const MIN_WAIT_MS = 500
const MAX_WAIT_MS = 10_000
const MAX_TOTAL_WAIT_MS = 30_000

let waiting = 0

export interface SpentBudget {
  scope: 'voter' | 'network'
  allowed: number
  windowSecs: number
  until: Date
}

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly retryAfterMs?: number,
    readonly budget?: SpentBudget,
  ) {
    super(message)
    this.name = 'ApiError'
  }

  get notFound(): boolean {
    return this.status === 404
  }

  get rateLimited(): boolean {
    return this.status === 429
  }
}

interface Send {
  method?: 'GET' | 'POST' | 'DELETE'
  body?: unknown
  credentials?: RequestCredentials
}

async function request<T>(path: string, signal?: AbortSignal, send: Send = {}): Promise<T> {
  let waited = 0

  const headers: Record<string, string> = { accept: 'application/json' }
  if (send.body !== undefined) headers['content-type'] = 'application/json'

  for (let attempt = 0; ; attempt++) {
    const response = await fetch(path, {
      signal,
      headers,
      method: send.method ?? 'GET',
      body: send.body === undefined ? undefined : JSON.stringify(send.body),
      credentials: send.credentials ?? 'omit',
    })

    if (response.status !== 429) {
      if (!response.ok) {
        throw new ApiError(await errorMessage(response), response.status)
      }
      return (await response.json()) as T
    }

    const spent = await budget(response)
    if (spent) {
      throw new ApiError('Vote budget spent.', 429, spent.until.getTime() - Date.now(), spent)
    }

    const wait = retryAfterMs(response)
    if (attempt + 1 >= MAX_ATTEMPTS || waited + wait > MAX_TOTAL_WAIT_MS) {
      throw new ApiError(rateLimitMessage(wait), 429, wait)
    }

    waited += wait
    await hold(wait, signal)
  }
}

async function hold(ms: number, signal?: AbortSignal): Promise<void> {
  waiting += 1
  throttled.value = true
  try {
    await sleep(ms, signal)
  } finally {
    waiting -= 1
    if (waiting === 0) throttled.value = false
  }
}

const OVER_BUDGET = 'over_budget'

async function budget(response: Response): Promise<SpentBudget | null> {
  try {
    const body = (await response.clone().json()) as {
      code?: string
      scope?: 'voter' | 'network'
      allowed?: number
      window_secs?: number
      retry_after?: number
    }
    if (body.code !== OVER_BUDGET || typeof body.retry_after !== 'number') return null

    return {
      scope: body.scope === 'network' ? 'network' : 'voter',
      allowed: body.allowed ?? 0,
      windowSecs: body.window_secs ?? 0,
      until: new Date(Date.now() + body.retry_after * 1000),
    }
  } catch {
    return null
  }
}

function retryAfterMs(response: Response): number {
  const header = response.headers.get('retry-after') ?? response.headers.get('x-ratelimit-after')
  const seconds = Number.parseInt(header ?? '', 10)
  const ms = Number.isFinite(seconds) ? seconds * 1000 : MIN_WAIT_MS * 2
  return Math.min(Math.max(ms, MIN_WAIT_MS), MAX_WAIT_MS)
}

function rateLimitMessage(waitMs: number): string {
  const seconds = Math.ceil(waitMs / 1000)
  return `Too many requests at once. Give it ${seconds} second${seconds === 1 ? '' : 's'} and try again.`
}

async function errorMessage(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: string }
    if (body.error) return body.error
  } catch {}
  return response.statusText || `request failed with ${response.status}`
}

function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(new DOMException('Aborted', 'AbortError'))
      return
    }

    const timer = setTimeout(resolve, ms)
    signal?.addEventListener(
      'abort',
      () => {
        clearTimeout(timer)
        reject(new DOMException('Aborted', 'AbortError'))
      },
      { once: true },
    )
  })
}

function query(params: Record<string, string | number | boolean | undefined>): string {
  const search = new URLSearchParams()
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== '') search.set(key, String(value))
  }
  const encoded = search.toString()
  return encoded ? `?${encoded}` : ''
}

export interface ListOptions {
  from?: string
  to?: string
  kind?: KindFilter
  copyright?: boolean
  lost?: boolean
  cursor?: string
  limit?: number
  order?: SortOrder
}

export interface SearchOptions extends Omit<ListOptions, 'cursor' | 'order'> {
  sort?: 'relevance' | 'date'
  offset?: number
}

export interface DivergenceOptions {
  field?: string
  offset?: number
  limit?: number
}

export interface ResourceOptions {
  q?: string
  host?: string
  min_refs?: number
  credited?: boolean
  sort?: ResourceSort
  order?: SortOrder
  offset?: number
  limit?: number
}

export interface PictureOptions {
  q?: string
  min_appearances?: number
  retitled?: boolean
  sort?: PictureSort
  order?: SortOrder
  offset?: number
  limit?: number
}

export interface PuzzleOptions {
  day?: string
  rounds?: number
  from?: string
}

export interface WordOptions {
  q?: string
  min_total?: number
  max_total?: number
  sort?: WordSort
  order?: SortOrder
  offset?: number
  limit?: number
}

export const api = {
  latest: (signal?: AbortSignal) => request<ApodEntry>('/api/entry/latest', signal),

  entry: (date: string, signal?: AbortSignal) =>
    request<ApodEntry>(`/api/entry/${encodeURIComponent(date)}`, signal),

  entries: (options: ListOptions = {}, signal?: AbortSignal) =>
    request<Page<ApodSummary>>(`/api/entries${query({ ...options })}`, signal),

  search: (q: string, options: SearchOptions = {}, signal?: AbortSignal) =>
    request<SearchResults>(`/api/search${query({ q, ...options })}`, signal),

  onThisDay: (monthDay: string, signal?: AbortSignal) =>
    request<ApodSummary[]>(`/api/on-this-day/${monthDay}`, signal),

  random: (kind?: KindFilter, signal?: AbortSignal) =>
    request<ApodEntry>(`/api/random${query({ kind })}`, signal),

  status: (signal?: AbortSignal) => request<Status>('/api/status', signal),

  sky: (signal?: AbortSignal) => request<Sky>('/api/sky', signal),

  weather: (signal?: AbortSignal) => request<WeatherReport>('/api/sky/weather', signal),

  stats: (signal?: AbortSignal) => request<Stats>('/api/stats', signal),

  timeline: (signal?: AbortSignal) => request<Timeline>('/api/stats/timeline', signal),

  coverage: (signal?: AbortSignal) => request<Coverage>('/api/stats/coverage', signal),

  migration: (signal?: AbortSignal) => request<Migration>('/api/migration', signal),

  divergences: (options: DivergenceOptions = {}, signal?: AbortSignal) =>
    request<Listing<FieldDivergence>>(`/api/migration/divergences${query({ ...options })}`, signal),

  resources: (options: ResourceOptions = {}, signal?: AbortSignal) =>
    request<Listing<Resource>>(`/api/resources${query({ ...options })}`, signal),

  resourceHosts: (signal?: AbortSignal) => request<HostCount[]>('/api/resources/hosts', signal),

  resource: (id: number, offset = 0, limit = 30, signal?: AbortSignal) =>
    request<ResourceRefs>(`/api/resources/${id}${query({ offset, limit })}`, signal),

  pictures: (options: PictureOptions = {}, signal?: AbortSignal) =>
    request<Listing<Picture>>(`/api/pictures${query({ ...options })}`, signal),

  /** Any of the picture's dates addresses it, not only the one it is named after. */
  picture: (date: string, signal?: AbortSignal) =>
    request<PictureAppearances>(`/api/pictures/${encodeURIComponent(date)}`, signal),

  words: (options: WordOptions = {}, signal?: AbortSignal) =>
    request<Listing<Word>>(`/api/words${query({ ...options })}`, signal),

  word: (word: string, signal?: AbortSignal) =>
    request<WordUse>(`/api/words/${encodeURIComponent(word)}`, signal),

  games: {
    date: (options: PuzzleOptions = {}, signal?: AbortSignal) =>
      request<Puzzle<GamePicture>>(`/api/games/date${query({ ...options })}`, signal),

    order: (options: PuzzleOptions = {}, signal?: AbortSignal) =>
      request<Puzzle<OrderPair>>(`/api/games/order${query({ ...options })}`, signal),

    match: (options: PuzzleOptions = {}, signal?: AbortSignal) =>
      request<Puzzle<MatchRound>>(`/api/games/match${query({ ...options })}`, signal),

    words: (options: PuzzleOptions = {}, signal?: AbortSignal) =>
      request<Puzzle<WordsRound>>(`/api/games/words${query({ ...options })}`, signal),

    reveal: (tokens: string[], signal?: AbortSignal) =>
      request<Reveal[]>(`/api/games/reveal${query({ t: tokens.join(',') })}`, signal),

    answer: (round: string, pick: string, signal?: AbortSignal) =>
      request<MatchAnswer>(`/api/games/answer${query({ round, pick })}`, signal),

    known: (word: string, signal?: AbortSignal) =>
      request<KnownWord>(`/api/games/known${query({ w: word })}`, signal),

    picture: (token: string) => `/pic/${encodeURIComponent(token)}`,
  },

  gaps: (signal?: AbortSignal) => request<Gap[]>('/api/gaps', signal),

  rating: {
    ballot: (category: RatingCategory, signal?: AbortSignal) =>
      request<Ballot>(`/api/rating/ballot${query({ category })}`, signal, {
        credentials: 'same-origin',
      }),

    vote: (ballot: string, outcome: RatingOutcome, category: RatingCategory) =>
      request<Cast>('/api/rating/vote', undefined, {
        method: 'POST',
        body: { ballot, outcome, category },
        credentials: 'same-origin',
      }),

    board: (
      category: RatingCategory,
      options: { limit?: number; offset?: number } = {},
      signal?: AbortSignal,
    ) => request<Board>(`/api/rating/board${query({ category, ...options })}`, signal),

    terms: (signal?: AbortSignal) => request<RatingTerms>('/api/rating/terms', signal),

    forget: () =>
      request<Forgotten>('/api/rating/votes', undefined, {
        method: 'DELETE',
        credentials: 'same-origin',
      }),
  },
}
