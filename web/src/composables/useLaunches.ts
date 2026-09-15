import { computed, onScopeDispose, ref } from 'vue'
import { api } from '@/api/client'
import type { Launch, Launches } from '@/api/types'
import { IMMINENT_MS } from '@/utils/launches'

const CALM_MS = 120_000
const WATCHING_MS = 30_000

const data = ref<Launches | null>(null)
const failed = ref(false)
const fetchedAt = ref(0)

let inFlight: Promise<void> | null = null
let watchers = 0
let timer: ReturnType<typeof setTimeout> | undefined

function load(): Promise<void> {
  if (inFlight) return inFlight

  inFlight = api
    .launches()
    .then((next) => {
      data.value = next
      failed.value = false
      fetchedAt.value = Date.now()
    })
    .catch(() => {
      failed.value = true
    })
    .finally(() => {
      inFlight = null
    })

  return inFlight
}

function soonest(): number | null {
  const ahead = data.value?.upcoming ?? []
  for (const launch of ahead) {
    const at = new Date(launch.net).getTime()
    if (!Number.isNaN(at)) return at
  }
  return null
}

function pace(): number {
  const live = (data.value?.upcoming ?? []).some((launch: Launch) => launch.webcast_live)
  if (live) return WATCHING_MS

  const next = soonest()
  if (next !== null && next - Date.now() < IMMINENT_MS) return WATCHING_MS
  return CALM_MS
}

function schedule(): void {
  clearTimeout(timer)
  if (watchers < 1) return

  timer = setTimeout(() => {
    void load().then(schedule)
  }, pace())
}

function onVisible(): void {
  if (document.visibilityState !== 'visible') return
  if (Date.now() - fetchedAt.value < WATCHING_MS) return schedule()

  void load().then(schedule)
}

export function useLaunches() {
  if (!fetchedAt.value) void load().then(schedule)

  watchers += 1
  if (watchers === 1) document.addEventListener('visibilitychange', onVisible)
  schedule()

  onScopeDispose(() => {
    watchers -= 1
    if (watchers > 0) return

    clearTimeout(timer)
    document.removeEventListener('visibilitychange', onVisible)
  })

  return {
    data,
    failed,
    fetchedAt,
    feed: computed(() => data.value?.feeds.find((state) => state.name === 'launches') ?? null),
    refresh: () => load().then(schedule),
  }
}
