import { computed, ref } from 'vue'
import { api } from '@/api/client'
import type { ApodSummary, PublishSchedule } from '@/api/types'

const latest = ref<ApodSummary | null>(null)
const entries = ref(0)
const publish = ref<PublishSchedule | null>(null)

let loaded = false
let inFlight: Promise<void> | null = null

function load(): Promise<void> {
  if (inFlight) return inFlight

  inFlight = api
    .status()
    .then((status) => {
      latest.value = status.latest
      entries.value = status.entries
      publish.value = status.publish
      loaded = true
    })
    .catch(() => {})
    .finally(() => {
      inFlight = null
    })

  return inFlight
}

export function useStatus() {
  if (!loaded) void load()

  return {
    latest,
    entries,
    publish,
    latestDate: computed(() => latest.value?.date ?? null),
    refresh: () => {
      loaded = false
      return load()
    },
  }
}

export function useLatestDate() {
  return useStatus().latestDate
}
