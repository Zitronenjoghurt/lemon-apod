import { ref } from 'vue'
import { api } from '@/api/client'

const latest = ref<string | null>(null)
let loaded = false
let inFlight: Promise<void> | null = null

export function useLatestDate() {
  if (!loaded && !inFlight) {
    inFlight = api
      .stats()
      .then((stats) => {
        latest.value = stats.latest
        loaded = true
      })
      .catch(() => {})
      .finally(() => {
        inFlight = null
      })
  }

  return latest
}
