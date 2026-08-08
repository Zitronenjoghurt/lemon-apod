import { computed, ref } from 'vue'
import { api } from '@/api/client'
import type { Sky } from '@/api/types'

const sky = ref<Sky | null>(null)
const failed = ref(false)

let loaded = false
let inFlight: Promise<void> | null = null

function load(): Promise<void> {
  if (inFlight) return inFlight

  inFlight = api
    .sky()
    .then((next) => {
      sky.value = next
      failed.value = false
      loaded = true
    })
    .catch(() => {
      failed.value = true
    })
    .finally(() => {
      inFlight = null
    })

  return inFlight
}

export function useSky() {
  if (!loaded) void load()

  return {
    sky,
    failed,
    visiblePlanets: computed(() =>
      (sky.value?.planets ?? []).filter(
        (planet) => planet.naked_eye && planet.visibility !== 'lost',
      ),
    ),
    refresh: () => {
      loaded = false
      return load()
    },
  }
}
