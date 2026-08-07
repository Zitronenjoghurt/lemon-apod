import { ref } from 'vue'
import type {
  ApodEntry,
  ApodSummary,
  HostCount,
  KindFilter,
  Listing,
  Page,
  Resource,
  ResourceRefs,
  ResourceSort,
  SearchResults,
  SortOrder,
  Stats,
  Timeline,
  Word,
  WordSort,
  WordUse,
} from './types'

export const throttled = ref(false)

const MAX_ATTEMPTS = 5
const MIN_WAIT_MS = 500
const MAX_WAIT_MS = 10_000
const MAX_TOTAL_WAIT_MS = 30_000

let waiting = 0

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly retryAfterMs?: number,
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

async function request<T>(path: string, signal?: AbortSignal): Promise<T> {
  let waited = 0

  for (let attempt = 0; ; attempt++) {
    const response = await fetch(path, { signal, headers: { accept: 'application/json' } })

    if (response.status !== 429) {
      if (!response.ok) {
        throw new ApiError(await errorMessage(response), response.status)
      }
      return (await response.json()) as T
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
  cursor?: string
  limit?: number
  order?: SortOrder
}

export interface SearchOptions extends Omit<ListOptions, 'cursor' | 'order'> {
  sort?: 'relevance' | 'date'
  offset?: number
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

  stats: (signal?: AbortSignal) => request<Stats>('/api/stats', signal),

  timeline: (signal?: AbortSignal) => request<Timeline>('/api/stats/timeline', signal),

  resources: (options: ResourceOptions = {}, signal?: AbortSignal) =>
    request<Listing<Resource>>(`/api/resources${query({ ...options })}`, signal),

  resourceHosts: (signal?: AbortSignal) => request<HostCount[]>('/api/resources/hosts', signal),

  resource: (id: number, offset = 0, limit = 30, signal?: AbortSignal) =>
    request<ResourceRefs>(`/api/resources/${id}${query({ offset, limit })}`, signal),

  words: (options: WordOptions = {}, signal?: AbortSignal) =>
    request<Listing<Word>>(`/api/words${query({ ...options })}`, signal),

  word: (word: string, signal?: AbortSignal) =>
    request<WordUse>(`/api/words/${encodeURIComponent(word)}`, signal),
}
