<script lang="ts" setup>
import {
  computed,
  nextTick,
  onActivated,
  onBeforeUnmount,
  onDeactivated,
  onMounted,
  ref,
  useTemplateRef,
  watch,
} from 'vue'
import { useRoute, useRouter } from 'vue-router'
import FeedItem from '@/components/FeedItem.vue'
import ReadFilter from '@/components/ReadFilter.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import type { ApodEntry, ApodSummary } from '@/api/types'
import { provideReadScope, useRead } from '@/composables/useRead'
import { FIRST_ENTRY, formatDate } from '@/utils/date'

defineOptions({ name: 'FeedView' })

type Mode = 'days' | 'random'

interface Item {
  date: string
  summary?: ApodSummary
  entry?: ApodEntry
}

const MODES: { label: string; value: Mode; icon: string }[] = [
  { label: 'Day by day', value: 'days', icon: 'pi pi-calendar' },
  { label: 'Random', value: 'random', icon: 'pi pi-sync' },
]

const PAGE_SIZE = 20
const RANDOM_ATTEMPTS = 12
const MAX_PAGES_PER_FILL = 5
const PRELOAD_PX = 1200
const TOP_BUTTON_AFTER = 3

const route = useRoute()
const router = useRouter()
provideReadScope('feed')
const { apply, active: filtered, filter } = useRead('feed')

const mode = ref<Mode>(readMode())
const from = ref<string | undefined>(readFrom())

function readMode(): Mode {
  return route.query.mode === 'random' ? 'random' : 'days'
}

function readFrom(): string | undefined {
  const raw = String(route.query.from ?? '')
  return /^\d{4}-\d{2}-\d{2}$/.test(raw) ? raw : undefined
}

function syncFromRoute(): boolean {
  const [nextMode, nextFrom] = [readMode(), readFrom()]
  if (nextMode === mode.value && nextFrom === from.value) return false

  mode.value = nextMode
  from.value = nextFrom
  return true
}

const items = ref<Item[]>([])
const cursor = ref<string | undefined>()
const loading = ref(false)
const done = ref(false)
const error = ref<string>()
const skipped = ref(0)

const stalled = ref(false)

const sentinel = useTemplateRef<HTMLElement>('sentinel')
const seen = new Set<string>()
let observer: IntersectionObserver | undefined
let filling = false
let generation = 0

function reset() {
  generation += 1
  items.value = []
  seen.clear()
  cursor.value = undefined
  done.value = false
  error.value = undefined
  skipped.value = 0
  stalled.value = false
}

async function fill() {
  if (filling) return
  filling = true
  stalled.value = false

  try {
    for (let page = 0; page < MAX_PAGES_PER_FILL; page += 1) {
      if (done.value || error.value || !withinReach()) return

      const before = items.value.length
      await loadMore()
      await nextTick()

      if (items.value.length > before && !withinReach()) return
    }

    stalled.value = !done.value && !error.value
  } finally {
    filling = false
  }
}

function withinReach(): boolean {
  const element = sentinel.value
  if (!element) return false
  return element.getBoundingClientRect().top < window.innerHeight + PRELOAD_PX
}

async function loadMore() {
  if (loading.value || done.value) return

  const run = generation
  loading.value = true
  error.value = undefined

  try {
    const batch = mode.value === 'random' ? await drawRandom() : await nextPage()
    if (run !== generation) return

    items.value = [...items.value, ...batch]
  } catch (thrown) {
    if (run !== generation) return
    error.value = thrown instanceof Error ? thrown.message : 'Something went wrong.'
  } finally {
    if (run === generation) loading.value = false
  }
}

async function nextPage(): Promise<Item[]> {
  const page = await api.entries({
    to: cursor.value ? undefined : from.value,
    cursor: cursor.value,
    limit: PAGE_SIZE,
    order: 'desc',
  })

  cursor.value = page.next_cursor
  if (!page.next_cursor) done.value = true

  const fresh = page.items.filter((item) => !seen.has(item.date))
  fresh.forEach((item) => seen.add(item.date))

  const kept = apply(fresh)
  skipped.value += fresh.length - kept.length

  return kept.map((summary) => ({ date: summary.date, summary }))
}

async function drawRandom(): Promise<Item[]> {
  for (let attempt = 0; attempt < RANDOM_ATTEMPTS; attempt += 1) {
    const entry = await api.random()
    if (seen.has(entry.date)) continue

    seen.add(entry.date)
    if (apply([entry]).length) return [{ date: entry.date, entry }]
    skipped.value += 1
  }

  done.value = true
  return []
}

watch([mode, from, filter], () => {
  reset()
  offset = 0
  if (!onScreen.value) return

  window.scrollTo(0, 0)
  void fill()
})

const onScreen = ref(false)
let offset = 0

const stopGuard = router.beforeEach((to) => {
  if (to.name === 'feed' || !onScreen.value) return
  offset = window.scrollY
  onScreen.value = false
})

onActivated(() => {
  onScreen.value = true
  if (syncFromRoute()) return

  if (!items.value.length && !done.value && !error.value) {
    void fill()
    return
  }

  restore()
})

const RESTORE_FRAMES = 30

function restore() {
  if (!offset) return

  let attempts = 0
  const apply = () => {
    window.scrollTo(0, offset)
    if (Math.abs(window.scrollY - offset) > 2 && attempts++ < RESTORE_FRAMES) {
      requestAnimationFrame(apply)
    }
  }
  requestAnimationFrame(apply)
}

onDeactivated(() => {
  onScreen.value = false
})

onMounted(() => {
  observer = new IntersectionObserver(
    ([entry]) => {
      if (entry?.isIntersecting) void fill()
    },
    { rootMargin: `${PRELOAD_PX}px 0px` },
  )
  if (sentinel.value) observer.observe(sentinel.value)
})

onBeforeUnmount(() => {
  observer?.disconnect()
  stopGuard()
})

function selectMode(next: Mode | null) {
  mode.value = next ?? 'days'
  router.replace({ name: 'feed', query: next === 'random' ? { mode: 'random' } : {} })
}

const showTopButton = computed(() => items.value.length > TOP_BUTTON_AFTER)

function toTop() {
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

const endNote = computed(() => {
  if (mode.value === 'random') {
    return filtered.value
      ? 'Random ran out of entries the filter keeps. Switch it back to All for more.'
      : 'That is enough randomness for one sitting. Reload for more.'
  }
  return `That is the whole archive, back to ${formatDate(FIRST_ENTRY)}.`
})
</script>

<template>
  <div class="stack feed">
    <header class="stack head">
      <div class="row justify">
        <h1>Feed</h1>
        <SelectButton
          :allow-empty="false"
          :model-value="mode"
          :options="MODES"
          aria-labelledby="mode-label"
          option-label="label"
          option-value="value"
          size="small"
          @update:model-value="selectMode"
        />
        <span id="mode-label" class="sr-only">Feed order</span>
      </div>

      <div class="row justify controls">
        <p class="muted note">
          {{
            mode === 'random'
              ? 'A different entry every time you scroll.'
              : from
                ? `Reading back from ${formatDate(from)}.`
                : 'Reading back from the newest entry.'
          }}
          Entries are marked read as you scroll past them.
        </p>
        <ReadFilter />
      </div>
    </header>

    <RetryNotice v-if="error" :busy="loading" :message="error" @retry="fill" />

    <FeedItem
      v-for="item in items"
      :key="item.date"
      :date="item.date"
      :preloaded="item.entry"
      :summary="item.summary"
    />

    <div v-if="loading" aria-busy="true" aria-label="Loading more entries" class="card placeholder">
      <Skeleton height="0.8rem" width="9rem" />
      <Skeleton height="1.6rem" width="55%" />
      <Skeleton height="16rem" width="100%" />
    </div>

    <p v-if="skipped && filtered" aria-live="polite" class="muted skipped">
      {{ skipped }} {{ skipped === 1 ? 'entry' : 'entries' }} skipped by the read filter.
    </p>

    <div v-if="stalled && !loading" class="more">
      <p class="muted">Nothing the filter keeps in the last few pages.</p>
      <Button
        icon="pi pi-chevron-down"
        label="Keep looking"
        outlined
        severity="secondary"
        @click="fill"
      />
    </div>

    <p v-if="done && !loading" class="muted end">{{ endNote }}</p>

    <div ref="sentinel" aria-hidden="true" class="sentinel" />

    <Transition name="fade">
      <Button
        v-if="showTopButton"
        v-tooltip.left="'Back to top'"
        aria-label="Back to top"
        class="to-top"
        icon="pi pi-arrow-up"
        rounded
        severity="secondary"
        @click="toTop"
      />
    </Transition>
  </div>
</template>

<style scoped>
.feed {
  max-width: 52rem;
  margin-inline: auto;
  gap: 1.5rem;
}

.head {
  gap: 0.6rem;
}

h1 {
  font-size: 1.6rem;
}

.justify {
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.controls {
  align-items: flex-start;
}

.note {
  margin: 0;
  font-size: 0.85rem;
  text-wrap: pretty;
}

.placeholder {
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
  padding: 1.1rem;
}

.skipped,
.end {
  text-align: center;
  font-size: 0.88rem;
  margin: 0;
  text-wrap: balance;
}

.more {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.7rem;
  padding-block: 1rem;
  text-align: center;
}

.more p {
  margin: 0;
  font-size: 0.88rem;
}

.end {
  padding-block: 1.5rem;
}

.sentinel {
  height: 1px;
}

.to-top {
  position: fixed;
  right: 1rem;
  bottom: 1.25rem;
  z-index: 4;
  box-shadow: var(--shadow);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}

@media (max-width: 40rem) {
  .feed {
    gap: 1.1rem;
  }

  .note {
    max-width: none;
  }
}
</style>
