<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import CalendarMonth from '@/components/CalendarMonth.vue'
import EntryGrid from '@/components/EntryGrid.vue'
import ReadFilter from '@/components/ReadFilter.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import type { ApodSummary } from '@/api/types'
import { useLatestDate } from '@/composables/useLatestDate'
import { useRead } from '@/composables/useRead'
import { FIRST_ENTRY, month as monthOf, year as yearOf } from '@/utils/date'

const FIRST_YEAR = yearOf(FIRST_ENTRY)
const FIRST_MONTH = monthOf(FIRST_ENTRY)
const PAGE_SIZE = 60

type View = 'grid' | 'calendar'

const VIEWS: { value: View; icon: string; label: string }[] = [
  { value: 'grid', icon: 'pi pi-th-large', label: 'Grid' },
  { value: 'calendar', icon: 'pi pi-calendar', label: 'Calendar' },
]

const MONTHS = Array.from({ length: 12 }, (_, index) => ({
  label: new Date(Date.UTC(2000, index, 1)).toLocaleString(undefined, { month: 'long' }),
  value: index + 1,
}))

const route = useRoute()
const router = useRouter()

const latest = useLatestDate()
const { apply, active: filtered } = useRead()

const newestYear = computed(() => (latest.value ? yearOf(latest.value) : null))

const year = computed(() =>
  route.params.year ? Number(route.params.year) : (newestYear.value ?? null),
)
const month = computed(() => (route.params.month ? Number(route.params.month) : null))
const view = computed<View>(() => (route.query.view === 'calendar' ? 'calendar' : 'grid'))

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

function go(nextYear: number, nextMonth: number | null, replace = false) {
  const path = nextMonth
    ? `/archive/${nextYear}/${String(nextMonth).padStart(2, '0')}`
    : `/archive/${nextYear}`

  const target = { path, query: view.value === 'calendar' ? { view: 'calendar' } : {} }
  void (replace ? router.replace(target) : router.push(target))
}

function goToYear(value: number | null) {
  if (value) go(value, month.value)
}

function goToMonth(value: number | null) {
  if (year.value) go(year.value, value)
}

function selectView(value: View | null) {
  router.replace({
    path: route.path,
    query: value === 'calendar' ? { view: 'calendar' } : {},
  })
}

const AT_FIRST = FIRST_YEAR * 12 + FIRST_MONTH

const atLatest = computed(() =>
  latest.value ? yearOf(latest.value) * 12 + monthOf(latest.value) : Number.POSITIVE_INFINITY,
)

function stepped(delta: number): { year: number; month: number | null } | null {
  if (!year.value) return null

  if (!month.value) {
    const next = year.value + delta
    return next >= FIRST_YEAR && next <= (newestYear.value ?? next)
      ? { year: next, month: null }
      : null
  }

  const at = year.value * 12 + month.value + delta
  if (at < AT_FIRST || at > atLatest.value) return null

  return { year: Math.floor((at - 1) / 12), month: ((at - 1) % 12) + 1 }
}

const previous = computed(() => stepped(-1))
const next = computed(() => stepped(1))

function step(delta: number) {
  const target = stepped(delta)
  if (target) go(target.year, target.month)
}

const entries = ref<ApodSummary[]>([])
const cursor = ref<string | undefined>()
const loading = ref(false)
const loadingMore = ref(false)
const error = ref<string>()

const shown = computed(() => apply(entries.value))
const hidden = computed(() => entries.value.length - shown.value.length)

let controller: AbortController | undefined
let failedAppend = false

function retry() {
  void load(failedAppend)
}

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
    failedAppend = append
    error.value = thrown instanceof Error ? thrown.message : 'Something went wrong.'
  } finally {
    if (!signal.aborted) {
      loading.value = false
      loadingMore.value = false
    }
  }
}

watch(range, () => load(false), { immediate: true })
watch(
  [view, year, month],
  () => {
    if (view.value !== 'calendar' || !year.value || month.value) return
    const fallback = months.value.at(-1)?.value
    if (fallback) go(year.value, fallback, true)
  },
  { immediate: true },
)

const periodLabel = computed(() => {
  if (!year.value) return ''
  const name = MONTHS.find((entry) => entry.value === month.value)?.label
  return name ? `${name} ${year.value}` : String(year.value)
})

function labelFor(target: { year: number; month: number | null } | null): string {
  if (!target) return ''
  const name = MONTHS.find((entry) => entry.value === target.month)?.label
  return name ? `${name} ${target.year}` : String(target.year)
}

const countLabel = computed(() => {
  const loaded = entries.value.length
  const noun = loaded === 1 ? 'entry' : 'entries'
  const suffix = filtered.value && hidden.value ? `, ${hidden.value} filtered out` : ''
  return cursor.value
    ? `${loaded} ${noun} so far from ${periodLabel.value}${suffix}`
    : `All ${loaded} ${noun} from ${periodLabel.value}${suffix}`
})
</script>

<template>
  <div class="stack">
    <header class="stack head">
      <div class="row justify">
        <h1>Archive</h1>
        <SelectButton
          :allow-empty="false"
          :model-value="view"
          :options="VIEWS"
          aria-labelledby="view-label"
          option-value="value"
          size="small"
          @update:model-value="selectView"
        >
          <template #option="{ option }">
            <i :class="option.icon" aria-hidden="true" />
            <span class="view-label">{{ option.label }}</span>
          </template>
        </SelectButton>
        <span id="view-label" class="sr-only">Layout</span>
      </div>

      <div class="row pickers">
        <Button
          v-tooltip.bottom="previous ? labelFor(previous) : undefined"
          :aria-label="`Earlier: ${labelFor(previous) || 'nothing before this'}`"
          :disabled="!previous"
          icon="pi pi-chevron-left"
          outlined
          severity="secondary"
          @click="step(-1)"
        />

        <Select
          :model-value="year"
          :options="years"
          aria-label="Year"
          class="year"
          placeholder="Year"
          @update:model-value="goToYear"
        />
        <Select
          :model-value="month"
          :options="months"
          :show-clear="view === 'grid'"
          aria-label="Month"
          class="month"
          option-label="label"
          option-value="value"
          placeholder="All months"
          @update:model-value="goToMonth"
        />

        <Button
          v-tooltip.bottom="next ? labelFor(next) : undefined"
          :aria-label="`Later: ${labelFor(next) || 'nothing after this'}`"
          :disabled="!next"
          icon="pi pi-chevron-right"
          outlined
          severity="secondary"
          @click="step(1)"
        />

        <ReadFilter class="read" />
      </div>
    </header>

    <RetryNotice v-if="error" :busy="loading || loadingMore" :message="error" @retry="retry" />

    <template v-if="!error || entries.length">
      <CalendarMonth
        v-if="view === 'calendar' && year && month"
        :entries="shown"
        :loading="loading"
        :month="month"
        :year="year"
      />

      <Skeleton v-else-if="view === 'calendar'" height="22rem" width="100%" />

      <EntryGrid
        v-else
        :empty="
          filtered && hidden
            ? `Every entry loaded from ${periodLabel} is filtered out. Load more, or switch the filter back to All.`
            : `Nothing archived for ${periodLabel || 'this period'} yet.`
        "
        :entries="shown"
        :loading="loading || !year"
      />
    </template>

    <div v-if="!loading && entries.length" class="more">
      <p aria-live="polite" class="muted count">{{ countLabel }}</p>
      <Button
        v-if="cursor"
        :loading="loadingMore"
        icon="pi pi-chevron-down"
        label="Load more"
        outlined
        severity="secondary"
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

.justify {
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.view-label {
  margin-left: 0.35rem;
}

.pickers {
  gap: 0.6rem;
  flex-wrap: wrap;
}

.year {
  min-width: 8rem;
}

.month {
  min-width: 10rem;
}

.read {
  margin-left: auto;
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

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}

@media (max-width: 46rem) {
  .read {
    margin-left: 0;
    width: 100%;
  }
}

@media (max-width: 30rem) {
  .view-label {
    display: none;
  }

  .year,
  .month {
    flex: 1;
    min-width: 0;
  }
}
</style>
