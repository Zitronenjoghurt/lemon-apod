<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import CalendarMonth from '@/components/CalendarMonth.vue'
import EntryGrid from '@/components/EntryGrid.vue'
import ReadFilter from '@/components/ReadFilter.vue'
import ReadProgress from '@/components/ReadProgress.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import type { ApodSummary } from '@/api/types'
import { useArrowKeys } from '@/composables/useArrowKeys'
import { useCoverage } from '@/composables/useCoverage'
import { usePreferences } from '@/composables/usePreferences'
import { useLatestDate } from '@/composables/useStatus'
import { provideReadScope, useRead } from '@/composables/useRead'
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
provideReadScope('archive')
const { apply, active: filtered, countIn } = useRead('archive')
const { archiveView } = usePreferences()
const coverage = useCoverage()

const newestYear = computed(() => (latest.value ? yearOf(latest.value) : null))

const year = computed(() =>
  route.params.year ? Number(route.params.year) : (newestYear.value ?? null),
)
const month = computed(() => (route.params.month ? Number(route.params.month) : null))

const view = computed<View>(() => {
  if (route.query.view === 'calendar') return 'calendar'
  if (route.query.view === 'grid') return 'grid'
  return archiveView.value
})

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
  const query = route.query.view ? { view: String(route.query.view) } : {}
  void (replace ? router.replace({ path, query }) : router.push({ path, query }))
}

function goToYear(value: number | null) {
  if (value) go(value, month.value)
}

function goToMonth(value: number | null) {
  if (year.value) go(year.value, value)
}

function selectView(value: View | null) {
  if (!value) return
  archiveView.value = value
  if (route.query.view) void router.replace({ path: route.path, query: {} })
}

const AT_FIRST = FIRST_YEAR * 12 + FIRST_MONTH

const atLatest = computed(() =>
  latest.value ? yearOf(latest.value) * 12 + monthOf(latest.value) : Number.POSITIVE_INFINITY,
)

type Period = { year: number; month: number | null }

function steppedMonth(delta: number): Period | null {
  if (!year.value || !month.value) return null

  const at = year.value * 12 + month.value + delta
  if (at < AT_FIRST || at > atLatest.value) return null

  return { year: Math.floor((at - 1) / 12), month: ((at - 1) % 12) + 1 }
}

function steppedYear(delta: number): Period | null {
  if (!year.value) return null

  const target = year.value + delta
  if (target < FIRST_YEAR || target > (newestYear.value ?? target)) return null
  if (!month.value) return { year: target, month: null }

  const at = Math.min(Math.max(target * 12 + month.value, AT_FIRST), atLatest.value)
  if (Math.floor((at - 1) / 12) !== target) return null

  return { year: target, month: ((at - 1) % 12) + 1 }
}

const previousMonth = computed(() => steppedMonth(-1))
const nextMonth = computed(() => steppedMonth(1))
const previousYear = computed(() => steppedYear(-1))
const nextYear = computed(() => steppedYear(1))

function goTo(target: Period | null) {
  if (target) go(target.year, target.month)
}

function localDate(iso: string): Date {
  const [on, of, day] = iso.split('-').map(Number)
  return new Date(on!, of! - 1, day!)
}

const oldestDay = localDate(FIRST_ENTRY)
const newestDay = computed(() => (latest.value ? localDate(latest.value) : new Date()))

const picked = ref<Date | null>(null)

function toIso(date: Date): string {
  const of = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${date.getFullYear()}-${of}-${day}`
}

function openDate(date: Date) {
  void router.push(`/${toIso(date)}`)
}

function browse(nextYear: number, nextMonth: number) {
  const at = Math.min(Math.max(nextYear * 12 + nextMonth, AT_FIRST), atLatest.value)
  go(Math.floor((at - 1) / 12), ((at - 1) % 12) + 1, true)
}

function onMonthChange({ month: changed, year: changedYear }: { month: number; year: number }) {
  browse(changedYear, changed)
}

function onYearChange({ month: changed, year: changedYear }: { month: number; year: number }) {
  browse(changedYear, changed + 1)
}

useArrowKeys({
  left: () => goTo(month.value ? previousMonth.value : previousYear.value),
  right: () => goTo(month.value ? nextMonth.value : nextYear.value),
  shiftLeft: () => goTo(previousYear.value),
  shiftRight: () => goTo(nextYear.value),
})

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

watch(
  [year, month],
  ([shownYear, shownMonth]) => {
    if (!shownYear || !shownMonth) return
    const on = picked.value
    if (on && on.getFullYear() === shownYear && on.getMonth() + 1 === shownMonth) return
    picked.value = new Date(shownYear, shownMonth - 1, 1)
  },
  { immediate: true },
)

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

function labelFor(target: Period | null): string {
  if (!target) return ''
  const name = MONTHS.find((entry) => entry.value === target.month)?.label
  return name ? `${name} ${target.year}` : String(target.year)
}

const periodPrefix = computed(() => {
  if (!year.value) return null
  return month.value ? `${year.value}-${String(month.value).padStart(2, '0')}` : String(year.value)
})

const periodRead = computed(() => (periodPrefix.value ? countIn(periodPrefix.value) : 0))

const periodTotal = computed(() => {
  if (!year.value) return undefined
  return month.value ? coverage.forMonth(year.value, month.value) : coverage.forYear(year.value)
})

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
        <div class="row stepper">
          <Button
            v-tooltip.bottom="previousYear ? labelFor(previousYear) : undefined"
            :aria-label="`Earlier year: ${labelFor(previousYear) || 'nothing before this'}`"
            :disabled="!previousYear"
            icon="pi pi-chevron-left"
            outlined
            severity="secondary"
            @click="goTo(previousYear)"
          />
          <Select
            :model-value="year"
            :options="years"
            aria-label="Year"
            class="year"
            placeholder="Year"
            @update:model-value="goToYear"
          />
          <Button
            v-tooltip.bottom="nextYear ? labelFor(nextYear) : undefined"
            :aria-label="`Later year: ${labelFor(nextYear) || 'nothing after this'}`"
            :disabled="!nextYear"
            icon="pi pi-chevron-right"
            outlined
            severity="secondary"
            @click="goTo(nextYear)"
          />
        </div>

        <div class="row stepper">
          <Button
            v-tooltip.bottom="previousMonth ? labelFor(previousMonth) : undefined"
            :aria-label="`Earlier month: ${labelFor(previousMonth) || 'nothing before this'}`"
            :disabled="!previousMonth"
            icon="pi pi-chevron-left"
            outlined
            severity="secondary"
            @click="goTo(previousMonth)"
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
            v-tooltip.bottom="nextMonth ? labelFor(nextMonth) : undefined"
            :aria-label="`Later month: ${labelFor(nextMonth) || 'nothing after this'}`"
            :disabled="!nextMonth"
            icon="pi pi-chevron-right"
            outlined
            severity="secondary"
            @click="goTo(nextMonth)"
          />
        </div>

        <DatePicker
          v-model="picked"
          :max-date="newestDay"
          :min-date="oldestDay"
          aria-label="Go to a date"
          class="jump"
          date-format="yy-mm-dd"
          icon-display="input"
          placeholder="Go to a date"
          show-icon
          @date-select="openDate"
          @month-change="onMonthChange"
          @year-change="onYearChange"
        />

        <ReadFilter class="read" />
      </div>

      <ReadProgress
        v-if="periodTotal"
        :label="periodLabel"
        :read="periodRead"
        :total="periodTotal"
      />
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
  gap: 0.6rem 0.9rem;
  flex-wrap: wrap;
  user-select: none;
  -webkit-user-select: none;
}

.stepper {
  gap: 0.35rem;
  flex-wrap: nowrap;
}

.year {
  min-width: 7rem;
}

.month {
  min-width: 9.5rem;
}

.jump {
  width: 10.5rem;
}

.jump :deep(input) {
  width: 100%;
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

@media (max-width: 52rem) {
  .stepper {
    flex: 1 1 14rem;
  }

  .year,
  .month {
    flex: 1;
    min-width: 0;
  }

  .jump {
    flex: 1 1 8rem;
    width: auto;
  }

  .read {
    flex: 1 1 9rem;
    margin-left: 0;
  }
}

@media (max-width: 30rem) {
  .view-label {
    display: none;
  }
}
</style>
