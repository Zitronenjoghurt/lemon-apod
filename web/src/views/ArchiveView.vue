<script setup lang="ts">
import { computed, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import EntryGrid from '@/components/EntryGrid.vue'
import { api } from '@/api/client'
import { useAsync } from '@/composables/useAsync'
import { useLatestDate } from '@/composables/useLatestDate'
import { year as yearOf } from '@/utils/date'

const FIRST_YEAR = 1995
const MONTHS = Array.from({ length: 12 }, (_, index) =>
  new Date(Date.UTC(2000, index, 1)).toLocaleString(undefined, { month: 'short' }),
)

const route = useRoute()
const latest = useLatestDate()

const newestYear = computed(() => (latest.value ? yearOf(latest.value) : null))

// No year in the URL lands on the newest one with every month shown, which is what someone
// opening the archive is nearly always after. Until the latest date arrives there is nothing
// to default to, and the grid shows its skeletons.
const year = computed(() =>
  route.params.year ? Number(route.params.year) : (newestYear.value ?? null),
)
const month = computed(() => (route.params.month ? Number(route.params.month) : null))

const years = computed(() => {
  const newest = newestYear.value ?? new Date().getUTCFullYear()
  return Array.from({ length: newest - FIRST_YEAR + 1 }, (_, index) => newest - index)
})

const range = computed(() => {
  if (!year.value) return null
  if (month.value) {
    const lastDay = new Date(Date.UTC(year.value, month.value, 0)).getUTCDate()
    const padded = String(month.value).padStart(2, '0')
    return { from: `${year.value}-${padded}-01`, to: `${year.value}-${padded}-${lastDay}` }
  }
  return { from: `${year.value}-01-01`, to: `${year.value}-12-31` }
})

const {
  data: page,
  loading,
  error,
  run,
} = useAsync((signal) =>
  api.entries(
    { from: range.value?.from, to: range.value?.to, limit: month.value ? 40 : 60, order: 'desc' },
    signal,
  ),
)

watch(
  range,
  (value) => {
    if (value) run()
  },
  { immediate: true },
)
</script>

<template>
  <div class="stack">
    <header class="stack head">
      <h1>Archive</h1>
      <nav class="row years" aria-label="Years">
        <RouterLink
          v-for="option in years"
          :key="option"
          :to="`/archive/${option}`"
          class="chip"
          :class="{ active: year === option }"
        >
          {{ option }}
        </RouterLink>
      </nav>

      <nav v-if="year" class="row months" aria-label="Months">
        <RouterLink :to="`/archive/${year}`" class="chip" :class="{ active: !month }"
          >All</RouterLink
        >
        <RouterLink
          v-for="(label, index) in MONTHS"
          :key="label"
          :to="`/archive/${year}/${String(index + 1).padStart(2, '0')}`"
          class="chip"
          :class="{ active: month === index + 1 }"
        >
          {{ label }}
        </RouterLink>
      </nav>
    </header>

    <p v-if="error" class="muted">{{ error }}</p>
    <EntryGrid
      v-else
      :entries="page?.items"
      :loading="loading || !year"
      empty="Nothing archived for this period yet."
    />

    <p v-if="page?.next_cursor" class="muted more">
      Showing the most recent entries for this period. Narrow it down by month to see the rest.
    </p>
  </div>
</template>

<style scoped>
.head {
  gap: 0.9rem;
}

h1 {
  font-size: 1.6rem;
}

.years,
.months {
  gap: 0.35rem;
}

.chip {
  font-size: 0.86rem;
  padding: 0.25rem 0.7rem;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--bg-elevated);
  color: var(--text-muted);
  text-decoration: none;
}

.chip:hover {
  color: var(--text);
}

.chip.active {
  color: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  background: color-mix(in srgb, var(--accent) 10%, var(--bg-elevated));
}

.empty,
.more {
  padding: 2rem 0;
  text-align: center;
  font-size: 0.9rem;
}
</style>
