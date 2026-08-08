import { computed, ref } from 'vue'

const STORAGE_KEY = 'apod:favorites'

const dates = ref<string[]>(load())

function load(): string[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    const parsed: unknown = raw ? JSON.parse(raw) : []
    return Array.isArray(parsed)
      ? parsed.filter((value): value is string => typeof value === 'string')
      : []
  } catch {
    return []
  }
}

function persist(): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(dates.value))
  } catch {}
}

export function hydrateFavorites(): void {
  dates.value = load()
}

export function useFavorites() {
  function isFavorite(date: string): boolean {
    return dates.value.includes(date)
  }

  function toggle(date: string): void {
    dates.value = isFavorite(date)
      ? dates.value.filter((entry) => entry !== date)
      : [...dates.value, date]
    persist()
  }

  function clear(): void {
    dates.value = []
    persist()
  }

  return {
    favorites: computed(() => [...dates.value].sort((a, b) => b.localeCompare(a))),
    count: computed(() => dates.value.length),
    isFavorite,
    toggle,
    clear,
  }
}
