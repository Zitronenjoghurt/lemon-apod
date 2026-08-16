import { computed, inject, type InjectionKey, provide, ref } from 'vue'

const READ_KEY = 'apod:read'
const LEGACY_FILTER_KEY = 'apod:read-filter'

export type ReadFilter = 'all' | 'unread' | 'read'
export type ReadScope = 'feed' | 'archive' | 'search' | 'favorites'
export const READ_SCOPES: ReadScope[] = ['feed', 'archive', 'search', 'favorites']

export function filterKey(scope: ReadScope): string {
  return `${LEGACY_FILTER_KEY}:${scope}`
}

export const READ_FILTERS: { label: string; value: ReadFilter; icon: string }[] = [
  { label: 'All', value: 'all', icon: 'pi pi-list' },
  { label: 'Unread', value: 'unread', icon: 'pi pi-circle-fill' },
  { label: 'Read', value: 'read', icon: 'pi pi-check' },
]

const SCOPE: InjectionKey<ReadScope> = Symbol('read-scope')

export function provideReadScope(scope: ReadScope): void {
  provide(SCOPE, scope)
}

const dates = ref<Set<string>>(loadDates())
const filters = ref<Record<ReadScope, ReadFilter>>(loadFilters())

function loadDates(): Set<string> {
  try {
    const raw = localStorage.getItem(READ_KEY)
    const parsed: unknown = raw ? JSON.parse(raw) : []
    return new Set(
      Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === 'string') : [],
    )
  } catch {
    return new Set()
  }
}

function loadFilters(): Record<ReadScope, ReadFilter> {
  const held = {} as Record<ReadScope, ReadFilter>
  for (const scope of READ_SCOPES) held[scope] = loadFilter(scope)
  return held
}

function loadFilter(scope: ReadScope): ReadFilter {
  const stored = localStorage.getItem(filterKey(scope)) ?? localStorage.getItem(LEGACY_FILTER_KEY)
  return stored === 'unread' || stored === 'read' ? stored : 'all'
}

function persist(): void {
  try {
    localStorage.setItem(READ_KEY, JSON.stringify([...dates.value].sort()))
  } catch {}
}

export function hydrateRead(): void {
  dates.value = loadDates()
  filters.value = loadFilters()
}

export function useRead(scope?: ReadScope) {
  const which = scope ?? inject<ReadScope | undefined>(SCOPE, undefined)

  const admitted = new Set<string>()
  let admittedFor = which ? filters.value[which] : 'all'

  const filter = computed<ReadFilter>({
    get: () => (which ? filters.value[which] : 'all'),
    set: (next) => {
      if (!which) return

      filters.value = { ...filters.value, [which]: next }
      try {
        localStorage.setItem(filterKey(which), next)
      } catch {}
    },
  })

  function isRead(date: string): boolean {
    return dates.value.has(date)
  }

  function markRead(date: string): void {
    if (!dates.value.has(date)) {
      dates.value.add(date)
      persist()
    }
  }

  function markUnread(date: string): void {
    if (dates.value.delete(date)) persist()
  }

  function toggleRead(date: string): boolean {
    const wasRead = isRead(date)
    if (wasRead) markUnread(date)
    else markRead(date)
    return !wasRead
  }

  function clear(): void {
    dates.value.clear()
    admitted.clear()
    persist()
  }

  function countIn(prefix?: string): number {
    if (!prefix) return dates.value.size

    let found = 0
    for (const date of dates.value) {
      if (date.startsWith(prefix)) found += 1
    }
    return found
  }

  function matches(date: string): boolean {
    if (filter.value === 'unread') return !isRead(date)
    if (filter.value === 'read') return isRead(date)
    return true
  }

  function dimmed(date: string): boolean {
    return filter.value !== 'all' && !matches(date)
  }

  function apply<T extends { date: string }>(entries: T[]): T[] {
    if (filter.value === 'all') return entries

    if (admittedFor !== filter.value) {
      admitted.clear()
      admittedFor = filter.value
    }

    const kept = entries.filter((entry) => matches(entry.date) || admitted.has(entry.date))
    for (const entry of kept) admitted.add(entry.date)
    return kept
  }

  return {
    filter,
    active: computed(() => filter.value !== 'all'),
    count: computed(() => dates.value.size),
    isRead,
    markRead,
    markUnread,
    toggleRead,
    clear,
    countIn,
    matches,
    dimmed,
    apply,
  }
}
