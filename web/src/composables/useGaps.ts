import { computed, ref } from 'vue'
import { api } from '@/api/client'
import type { Gap } from '@/api/types'

const gaps = ref<Gap[]>([])
const arrived = ref(false)
let inFlight: Promise<void> | null = null

function load(): Promise<void> {
  if (inFlight) return inFlight

  inFlight = api
    .gaps()
    .then((found) => {
      gaps.value = found
      arrived.value = true
    })
    .catch(() => {})
    .finally(() => {
      inFlight = null
    })

  return inFlight
}

export function useGaps() {
  if (!arrived.value) void load()

  return {
    gaps,
    loaded: computed(() => arrived.value),
    gapFor: (date: string) => computed(() => gaps.value.find((gap) => gap.date === date) ?? null),
  }
}

export function isGap(date: string): boolean {
  return gaps.value.some((gap) => gap.date === date)
}
