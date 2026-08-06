import { ref } from 'vue'
import { api } from '@/api/client'

const latest = ref<string | null>(null)
let inFlight: Promise<void> | null = null

export function useLatestDate() {
  inFlight ??= api
    .stats()
    .then((stats) => {
      latest.value = stats.latest
    })
    .catch(() => {})
  return latest
}
