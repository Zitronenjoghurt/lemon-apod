import { computed, ref } from 'vue'
import { api } from '@/api/client'
import type { MonthCount } from '@/api/types'

const months = ref<MonthCount[]>([])

let loaded = false
let inFlight: Promise<void> | null = null

function load(): void {
  if (loaded || inFlight) return

  inFlight = api
    .coverage()
    .then((coverage) => {
      months.value = coverage.months
      loaded = true
    })
    .catch(() => {})
    .finally(() => {
      inFlight = null
    })
}

export function useCoverage() {
  load()

  const byYear = computed(() => {
    const totals = new Map<number, number>()
    for (const month of months.value) {
      totals.set(month.year, (totals.get(month.year) ?? 0) + month.entries)
    }
    return totals
  })

  const byMonth = computed(
    () => new Map(months.value.map((month) => [`${month.year}-${month.month}`, month.entries])),
  )

  return {
    /// Undefined until the request lands, which is not the same as a period holding nothing.
    forYear: (year: number): number | undefined => byYear.value.get(year),
    forMonth: (year: number, month: number): number | undefined =>
      byMonth.value.get(`${year}-${month}`),
    total: computed(() => months.value.reduce((sum, month) => sum + month.entries, 0)),
    ready: computed(() => months.value.length > 0),
  }
}
