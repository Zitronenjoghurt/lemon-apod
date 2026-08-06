import { ref } from 'vue'
import type { ApodEntry, ApodSummary, MediaKind, Page, SearchResults, Stats } from './types'

export const throttled = ref(false)
const MAX_RETRY_WAIT_MS = 10_000

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message)
    this.name = 'ApiError'
  }

  get notFound(): boolean {
    return this.status === 404
  }
}

async function request<T>(path: string, signal?: AbortSignal): Promise<T> {
  for (let attempt = 0; attempt < 2; attempt++) {
    const response = await fetch(path, { signal, headers: { accept: 'application/json' } })

    if (response.status === 429 && attempt === 0) {
      const waitMs = retryAfterMs(response)
      if (waitMs > MAX_RETRY_WAIT_MS) break

      throttled.value = true
      try {
        await sleep(waitMs, signal)
      } finally {
        throttled.value = false
      }
      continue
    }

    if (!response.ok) {
      throw new ApiError(await errorMessage(response), response.status)
    }

    return (await response.json()) as T
  }

  throw new ApiError('Too many requests. Please slow down.', 429)
}

function retryAfterMs(response: Response): number {
  const header = response.headers.get('retry-after') ?? response.headers.get('x-ratelimit-after')
  const seconds = Number.parseInt(header ?? '', 10)
  return Number.isFinite(seconds) ? Math.max(seconds, 1) * 1000 : 1000
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
  kind?: MediaKind
  copyright?: boolean
  cursor?: string
  limit?: number
  order?: 'asc' | 'desc'
}

export interface SearchOptions extends Omit<ListOptions, 'cursor' | 'order'> {
  sort?: 'relevance' | 'date'
  offset?: number
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

  random: (kind?: MediaKind, signal?: AbortSignal) =>
    request<ApodEntry>(`/api/random${query({ kind })}`, signal),

  stats: (signal?: AbortSignal) => request<Stats>('/api/stats', signal),
}
