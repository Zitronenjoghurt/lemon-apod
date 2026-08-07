import { computed, ref, watch } from 'vue'

const READ_KEY = 'apod:read'
const FILTER_KEY = 'apod:read-filter'

export type ReadFilter = 'all' | 'unread' | 'read'

export const READ_FILTERS: { label: string; value: ReadFilter; icon: string }[] = [
  { label: 'All', value: 'all', icon: 'pi pi-list' },
  { label: 'Unread', value: 'unread', icon: 'pi pi-circle' },
  { label: 'Read', value: 'read', icon: 'pi pi-check-circle' },
]

const dates = ref<Set<string>>(loadDates())
const filter = ref<ReadFilter>(loadFilter())

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

function loadFilter(): ReadFilter {
  const stored = localStorage.getItem(FILTER_KEY)
  return stored === 'unread' || stored === 'read' ? stored : 'all'
}

function persist(): void {
  try {
    localStorage.setItem(READ_KEY, JSON.stringify([...dates.value].sort()))
  } catch {}
}

watch(filter, (next) => {
  try {
    localStorage.setItem(FILTER_KEY, next)
  } catch {}
})

export function useRead() {
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
    persist()
  }

  function matches(date: string): boolean {
    if (filter.value === 'unread') return !isRead(date)
    if (filter.value === 'read') return isRead(date)
    return true
  }

  function apply<T extends { date: string }>(entries: T[]): T[] {
    return filter.value === 'all' ? entries : entries.filter((entry) => matches(entry.date))
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
    matches,
    apply,
  }
}
