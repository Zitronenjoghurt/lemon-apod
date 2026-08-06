<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import EntryGrid from '@/components/EntryGrid.vue'
import { api } from '@/api/client'
import type { ApodSummary } from '@/api/types'
import { useLatestDate } from '@/composables/useLatestDate'
import { FIRST_ENTRY, month as monthOf, year as yearOf } from '@/utils/date'

const FIRST_YEAR = yearOf(FIRST_ENTRY)
const FIRST_MONTH = monthOf(FIRST_ENTRY)
const PAGE_SIZE = 60

const MONTHS = Array.from({ length: 12 }, (_, index) => ({
  label: new Date(Date.UTC(2000, index, 1)).toLocaleString(undefined, { month: 'long' }),
  value: index + 1,
}))

const route = useRoute()
const router = useRouter()

const latest = useLatestDate()

const newestYear = computed(() => (latest.value ? yearOf(latest.value) : null))

const year = computed(() =>
  route.params.year ? Number(route.params.year) : (newestYear.value ?? null),
)
const month = computed(() => (route.params.month ? Number(route.params.month) : null))

const years = computed(() => {
  const newest = newestYear.value ?? new Date().getUTCFullYear()
  return Array.from({ length: newest - FIRST_YEAR + 1 }, (_, index) => newest - index)
})

const months = computed(() => {
  if (!year.value) return MONTHS
  const from = year.value === FIRST_YEAR ? FIRST_MONTH : 1
  const to = latest.value && year.value === newestYear.value ? monthOf(latest.value) : MONTHS.length
  return MONTHS.filter((entry) => entry.value >= from && entry.value <= to)
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

function goToYear(value: number | null) {
  if (value) router.push(`/archive/${value}`)
}

function goToMonth(value: number | null) {
  if (!year.value) return
  router.push(
    value ? `/archive/${year.value}/${String(value).padStart(2, '0')}` : `/archive/${year.value}`,
  )
}

const entries = ref<ApodSummary[]>([])
const cursor = ref<string | undefined>()
const loading = ref(false)
const loadingMore = ref(false)
const error = ref<string>()

let controller: AbortController | undefined

async function load(append: boolean) {
  if (!range.value) return

  controller?.abort()
  controller = new AbortController()
  const { signal } = controller

  error.value = undefined
  if (append) {
    loadingMore.value = true
  } else {
    loading.value = true
    entries.value = []
    cursor.value = undefined
  }

  try {
    const page = await api.entries(
      {
        from: range.value.from,
        to: range.value.to,
        limit: PAGE_SIZE,
        order: 'desc',
        cursor: append ? cursor.value : undefined,
      },
      signal,
    )
    if (signal.aborted) return

    entries.value = append ? [...entries.value, ...page.items] : page.items
    cursor.value = page.next_cursor
  } catch (thrown) {
    if (signal.aborted || (thrown instanceof DOMException && thrown.name === 'AbortError')) return
    error.value = thrown instanceof Error ? thrown.message : 'Something went wrong.'
  } finally {
    if (!signal.aborted) {
      loading.value = false
      loadingMore.value = false
    }
  }
}

watch(range, () => load(false), { immediate: true })

const periodLabel = computed(() => {
  if (!year.value) return ''
  const name = MONTHS.find((entry) => entry.value === month.value)?.label
  return name ? `${name} ${year.value}` : String(year.value)
})

const countLabel = computed(() => {
  const shown = entries.value.length
  const noun = shown === 1 ? 'entry' : 'entries'
  return cursor.value
    ? `${shown} ${noun} so far from ${periodLabel.value}`
    : `All ${shown} ${noun} from ${periodLabel.value}`
})
</script>

<template>
  <div class="stack">
    <header class="stack head">
      <h1>Archive</h1>

      <div class="row pickers">
        <Select
          :model-value="year"
          :options="years"
          placeholder="Year"
          class="year"
          aria-label="Year"
          @update:model-value="goToYear"
        />
        <Select
          :model-value="month"
          :options="months"
          option-label="label"
          option-value="value"
          placeholder="All months"
          show-clear
          class="month"
          aria-label="Month"
          @update:model-value="goToMonth"
        />
      </div>
    </header>

    <Message v-if="error" severity="error" :closable="false">{{ error }}</Message>

    <EntryGrid
      v-else
      :entries="entries"
      :loading="loading || !year"
      :empty="`Nothing archived for ${periodLabel || 'this period'} yet.`"
    />

    <div v-if="!loading && entries.length" class="more">
      <p class="muted count" aria-live="polite">{{ countLabel }}</p>
      <Button
        v-if="cursor"
        label="Load more"
        icon="pi pi-chevron-down"
        severity="secondary"
        outlined
        :loading="loadingMore"
        @click="load(true)"
      />
    </div>
  </div>
</template>

<style scoped>
.head {
  gap: 0.9rem;
}

h1 {
  font-size: 1.6rem;
}

.pickers {
  gap: 0.6rem;
}

.year {
  min-width: 8rem;
}

.month {
  min-width: 10rem;
}

.more {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.75rem;
  padding-top: 0.5rem;
}

.count {
  margin: 0;
  font-size: 0.9rem;
}

@media (max-width: 30rem) {
  .pickers {
    flex-wrap: nowrap;
  }

  .year,
  .month {
    flex: 1;
    min-width: 0;
  }
}
</style>
