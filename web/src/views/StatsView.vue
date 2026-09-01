<script lang="ts" setup>
import { computed, onMounted, ref, useTemplateRef } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import RetryNotice from '@/components/RetryNotice.vue'
import WordDetail from '@/components/WordDetail.vue'
import YearChart from '@/components/YearChart.vue'
import { api } from '@/api/client'
import type { SortOrder, Word, WordSort } from '@/api/types'
import { useAsync } from '@/composables/useAsync'
import { useNarrow } from '@/composables/useNarrow'
import { daysBetween, formatDate } from '@/utils/date'

const PAGE_SIZE = 50
const DEBOUNCE_MS = 250

const SORTS: { label: string; value: `${WordSort}:${SortOrder}` }[] = [
  { label: 'Most used', value: 'total:desc' },
  { label: 'Least used', value: 'total:asc' },
  { label: 'In most entries', value: 'entries:desc' },
  { label: 'A to Z', value: 'word:asc' },
]

const route = useRoute()
const router = useRouter()
const { pageLinks } = useNarrow()

const {
  data: stats,
  error: statsError,
  loading: statsLoading,
  run: loadStats,
} = useAsync((signal) => api.stats(signal))

const {
  data: timeline,
  error: timelineError,
  loading: timelineLoading,
  run: loadTimeline,
} = useAsync((signal) => api.timeline(signal))

const gapsPopover = useTemplateRef<{ toggle: (event: Event) => void }>('gapsPopover')

const gapRuns = computed(() => {
  const runs: { from: string; to: string; days: number }[] = []

  for (const day of stats.value?.gap_dates ?? []) {
    const open = runs[runs.length - 1]

    if (open && daysBetween(open.to, day) === 1) {
      open.to = day
      open.days += 1
    } else {
      runs.push({ from: day, to: day, days: 1 })
    }
  }

  return runs
})

const query = ref('')
const sort = ref<`${WordSort}:${SortOrder}`>('total:desc')
const page = ref(1)

const {
  data: words,
  error: wordsError,
  loading: wordsLoading,
  run: loadWords,
} = useAsync((signal) => {
  const [by, order] = sort.value.split(':') as [WordSort, SortOrder]
  return api.words(
    {
      q: query.value || undefined,
      sort: by,
      order,
      offset: (page.value - 1) * PAGE_SIZE,
      limit: PAGE_SIZE,
    },
    signal,
  )
})

let debounce: ReturnType<typeof setTimeout> | undefined

function searchWords(resetPage: boolean) {
  if (resetPage) page.value = 1
  clearTimeout(debounce)
  debounce = setTimeout(loadWords, DEBOUNCE_MS)
}

function onPage(event: { page: number }) {
  page.value = event.page + 1
  void loadWords()
}

onMounted(() => {
  void loadStats()
  void loadTimeline()
  void loadWords()
})

const openWord = computed(() => String(route.query.word ?? '') || undefined)

function openDetail(word: Word) {
  void router.push({ query: { ...route.query, word: word.word } })
}

function closeDetail() {
  const query = { ...route.query }
  delete query.word
  void router.replace({ query })
}

const text = computed(() => stats.value?.text)
const catalogue = computed(() => stats.value?.resources)
const unmeasured = computed(() => stats.value != null && stats.value.text.measured === 0)
const years = computed(() => timeline.value?.years ?? [])

function series(pick: (year: (typeof years.value)[number]) => number) {
  return years.value.map((year) => ({ year: year.year, value: pick(year) }))
}

const lengthBands = computed(() => {
  const bands = stats.value?.text.lengths ?? []
  const peak = Math.max(...bands.map((band) => band.entries), 1)
  return bands.map((band) => ({
    label: band.to === undefined ? `${band.from}+` : `${band.from}–${band.to}`,
    entries: band.entries,
    width: band.entries === 0 ? 0 : Math.max((band.entries / peak) * 100, 1.5),
  }))
})

const kindShare = computed(() => {
  const rows = stats.value?.by_media_kind ?? []
  const total = rows.reduce((sum, row) => sum + row.count, 0)
  return rows.map((row) => ({ ...row, share: total ? row.count / total : 0 }))
})

const KIND_LABELS: Record<string, string> = {
  image_jpg: 'JPEG image',
  image_png: 'PNG image',
  image_gif: 'GIF image',
  video_mp4: 'MP4 video',
  youtube: 'YouTube',
  vimeo: 'Vimeo',
  embed: 'Embed',
  other: 'Other',
  none: 'None',
}

function round(value: number | undefined, decimals = 1): string {
  if (value === undefined) return 'n/a'
  return value.toLocaleString(undefined, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  })
}

function count(value: number | undefined): string {
  return value === undefined ? 'n/a' : value.toLocaleString()
}
</script>

<template>
  <div class="stack stats">
    <header class="stack head">
      <h1>Statistics</h1>
    </header>

    <RetryNotice v-if="statsError" :busy="statsLoading" :message="statsError" @retry="loadStats" />

    <Message v-else-if="unmeasured" :closable="false" severity="secondary">
      The archive has not been parsed by a build that counts words yet. Run
      <code>apod-archiver reparse --stale</code> and this fills in.
    </Message>

    <section v-if="stats" class="tiles">
      <div class="card tile">
        <span class="muted name">Entries</span>
        <strong class="value">{{ count(stats.entries) }}</strong>
        <span v-if="stats.first && stats.latest" class="muted foot">
          {{ formatDate(stats.first) }} to {{ formatDate(stats.latest) }}
        </span>
      </div>
      <div class="card tile">
        <span class="muted name">Words written</span>
        <strong class="value">{{ count(text?.total_words) }}</strong>
        <span class="muted foot">{{ count(text?.distinct_words) }} different ones</span>
      </div>
      <div class="card tile">
        <span class="muted name">Pictures</span>
        <strong class="value">{{ count(stats.thumbnails) }}</strong>
        <span class="muted foot">
          thumbnailed, {{ count(stats.entries - stats.thumbnails) }} failed
        </span>
      </div>
      <div class="card tile">
        <span class="muted name">Resources linked</span>
        <strong class="value">{{ count(catalogue?.resources) }}</strong>
        <span class="muted foot">
          across {{ count(catalogue?.hosts) }} sites,
          <RouterLink to="/resources">browse them</RouterLink>
        </span>
      </div>
      <div class="card tile">
        <span class="muted name">Under copyright</span>
        <strong class="value">
          {{ round((stats.copyright / Math.max(stats.entries, 1)) * 100, 0) }}%
        </strong>
        <span class="muted foot">
          {{ count(stats.copyright) }} entries, {{ count(stats.licensed) }} naming a licence
        </span>
      </div>
      <div class="card tile">
        <span class="muted name">Encores</span>
        <strong class="value">{{ count(stats.pictures.pictures) }}</strong>
        <span class="muted foot">
          shown again across {{ count(stats.pictures.entries) }} entries,
          <RouterLink to="/pictures">browse them</RouterLink>
        </span>
      </div>
      <div v-if="stats.pictures.most_shown" class="card tile">
        <span class="muted name">Most repeated</span>
        <strong class="value">{{ count(stats.pictures.most_shown_times) }}&times;</strong>
        <span class="muted foot">
          the same picture, from
          <RouterLink :to="`/pictures/${stats.pictures.most_shown}`">
            {{ formatDate(stats.pictures.most_shown) }}
          </RouterLink>
        </span>
      </div>
      <div class="card tile">
        <span class="muted name">Days missed</span>
        <strong class="value">{{ count(stats.gaps) }}</strong>
        <span v-if="!stats.gaps" class="muted foot">a picture every single day</span>
        <span v-else class="muted foot">
          days APOD published nothing,
          <button
            :aria-label="`Which ${stats.gaps} days APOD published nothing`"
            class="which"
            type="button"
            @click="gapsPopover?.toggle($event)"
          >
            see which
          </button>
        </span>
      </div>
    </section>

    <Popover ref="gapsPopover">
      <div class="gaps">
        <ul>
          <li v-for="run in gapRuns" :key="run.from">
            <span class="when">
              {{ formatDate(run.from) }}
              <template v-if="run.to !== run.from"> to {{ formatDate(run.to) }}</template>
            </span>
            <span class="tail">
              <span class="muted howmany">
                {{ run.days }} {{ run.days === 1 ? 'day' : 'days' }}
              </span>
              <RouterLink :to="`/${run.from}`" class="why">
                why
                <i aria-hidden="true" class="pi pi-angle-right" />
              </RouterLink>
            </span>
          </li>
        </ul>
      </div>
    </Popover>

    <section v-if="text && text.measured" class="card panel">
      <h2>Length distribution between all APODs</h2>
      <p class="muted lede">
        Half of them are between <strong>{{ count(text.p25_words) }}</strong> and
        <strong>{{ count(text.p75_words) }}</strong> words, with the median at
        <strong>{{ count(text.median_words) }}</strong
        >. The shortest is {{ count(text.min_words) }} and the longest {{ count(text.max_words) }}.
      </p>

      <ul v-if="lengthBands.length" class="bands">
        <li v-for="band in lengthBands" :key="band.label">
          <span class="band-name">{{ band.label }}</span>
          <span class="meter" role="presentation">
            <span :style="{ width: `${band.width}%` }" class="fill" />
          </span>
          <span class="tabular band-count">{{ count(band.entries) }}</span>
        </li>
      </ul>

      <div class="row extremes">
        <p v-if="text.shortest" class="muted">
          Shortest:
          <RouterLink :to="`/${text.shortest.date}`">{{ text.shortest.title }}</RouterLink>
          <span class="tabular"> ({{ count(text.shortest.word_count) }} words)</span>
        </p>
        <p v-if="text.longest" class="muted">
          Longest:
          <RouterLink :to="`/${text.longest.date}`">{{ text.longest.title }}</RouterLink>
          <span class="tabular"> ({{ count(text.longest.word_count) }} words)</span>
        </p>
      </div>
    </section>

    <section v-if="text && text.measured" class="card panel">
      <h2>What an average APOD consists of</h2>
      <dl class="facts">
        <div>
          <dt>Words</dt>
          <dd>{{ round(text.avg_words) }}</dd>
        </div>
        <div>
          <dt>Of them different</dt>
          <dd>{{ round(text.avg_unique_words) }}</dd>
        </div>
        <div>
          <dt>Characters</dt>
          <dd>{{ round(text.avg_chars, 0) }}</dd>
        </div>
        <div>
          <dt>Sentences</dt>
          <dd>{{ round(text.avg_sentences) }}</dd>
        </div>
        <div>
          <dt>Words per sentence</dt>
          <dd>{{ round(text.avg_words_per_sentence) }}</dd>
        </div>
        <div>
          <dt>Links out</dt>
          <dd>{{ round(text.avg_links) }}</dd>
        </div>
      </dl>
    </section>

    <section class="card panel">
      <h2>Over time</h2>

      <RetryNotice
        v-if="timelineError"
        :busy="timelineLoading"
        :message="timelineError"
        @retry="loadTimeline"
      />

      <div v-else-if="timelineLoading && !years.length" class="charts">
        <Skeleton v-for="index in 6" :key="index" height="9rem" width="100%" />
      </div>

      <div v-else-if="years.length" class="charts">
        <YearChart
          :decimals="1"
          :points="series((y) => y.avg_words)"
          kind="line"
          label="Average words per entry"
        />
        <YearChart
          :decimals="1"
          :points="series((y) => y.avg_words_per_sentence)"
          kind="line"
          label="Average words per sentence"
        />
        <YearChart
          :decimals="1"
          :points="series((y) => y.avg_links)"
          kind="line"
          label="Average links per entry"
        />
        <YearChart :points="series((y) => y.distinct_words)" label="Different words used" />
        <YearChart :points="series((y) => y.new_words)" label="Words used for the first time" />
        <YearChart :points="series((y) => y.videos)" label="Entries that were video" />
        <YearChart :points="series((y) => y.copyright)" label="Entries under copyright" />
      </div>

      <p v-else class="muted empty">
        No year has anything to plot yet. The charts fill in once the archive has entries.
      </p>
    </section>

    <section v-if="kindShare.length" class="card panel">
      <h2>What kinds of media were used</h2>
      <ul class="kinds">
        <li v-for="row in kindShare" :key="row.kind">
          <span class="kind-name">{{ KIND_LABELS[row.kind] ?? row.kind }}</span>
          <span class="meter" role="presentation">
            <span :style="{ width: `${Math.max(row.share * 100, 0.6)}%` }" class="fill" />
          </span>
          <span class="tabular kind-count">{{ count(row.count) }}</span>
        </li>
      </ul>
    </section>

    <section class="card panel">
      <h2>Every word</h2>
      <div class="row controls">
        <IconField class="word-search">
          <InputIcon class="pi pi-search" />
          <InputText
            v-model="query"
            aria-label="Search words"
            fluid
            placeholder="nebula, neb*, …"
            type="search"
            @input="searchWords(true)"
          />
        </IconField>
        <Select
          v-model="sort"
          :options="SORTS"
          aria-label="Order"
          class="word-sort"
          option-label="label"
          option-value="value"
          @update:model-value="searchWords(true)"
        />
      </div>

      <RetryNotice
        v-if="wordsError"
        :busy="wordsLoading"
        :message="wordsError"
        @retry="loadWords"
      />

      <p v-if="words" aria-live="polite" class="muted count">
        {{ count(words.total) }} {{ words.total === 1 ? 'word' : 'words' }} registered
        <template v-if="text?.used_once && !query">
          &middot; {{ count(text.used_once) }} of them were only used once
        </template>
      </p>

      <div v-if="wordsLoading && !words" class="stack lines">
        <Skeleton v-for="index in 6" :key="index" height="1.6rem" width="100%" />
      </div>

      <p v-else-if="words && !words.items.length" class="muted empty">
        No word in the archive matches your search.
      </p>

      <ul v-else-if="words" class="words">
        <li v-for="word in words.items" :key="word.word">
          <button
            v-tooltip.bottom="
              `Written ${count(word.total)} times, across ${count(word.entries)} entries`
            "
            class="word"
            type="button"
            @click="openDetail(word)"
          >
            <span class="text">{{ word.word }}</span>
            <span aria-hidden="true" class="leader" />
            <span class="tabular uses">{{ count(word.total) }}&times;</span>
            <span class="muted tabular entries">{{ count(word.entries) }}</span>
            <span class="sr-only">
              used {{ count(word.total) }} times in {{ count(word.entries) }} entries
            </span>
          </button>
        </li>
      </ul>

      <Paginator
        v-if="words && words.total > PAGE_SIZE"
        :first="(page - 1) * PAGE_SIZE"
        :page-link-size="pageLinks"
        :rows="PAGE_SIZE"
        :total-records="words.total"
        @page="onPage"
      />
    </section>

    <WordDetail :word="openWord" @close="closeDetail" />
  </div>
</template>

<style scoped>
.stats {
  gap: var(--space-6);
}

h1 {
  font-size: var(--text-xl);
}

h2 {
  font-size: var(--text-md);
  margin-bottom: var(--space-1);
}

.head {
  gap: var(--space-2);
}

.tiles {
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 13rem), 1fr));
}

.tile {
  padding: var(--space-4) var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-0);
}

.tile .name {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.tile .value {
  font-size: var(--text-xl);
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.02em;
  line-height: 1.2;
}

.tile .foot {
  font-size: var(--text-sm);
}

.which {
  padding: 0;
  border: 0;
  background: none;
  font: inherit;
  color: var(--accent);
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 0.15em;
}

.which:hover,
.which:focus-visible {
  text-decoration-thickness: 2px;
}

.gaps {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  max-width: 22rem;
}

.gaps .lede {
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.gaps ul {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.gaps li {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-5);
  font-size: var(--text-sm);
}

.gaps .when {
  text-wrap: pretty;
}

.gaps .tail {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-2);
  white-space: nowrap;
}

.gaps .howmany {
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.gaps .why {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-0);
  font-size: var(--text-sm);
  text-decoration: none;
}

.gaps .why:hover {
  text-decoration: underline;
}

.gaps .why i {
  font-size: 0.75em;
}

.panel {
  padding: var(--space-5) var(--space-5) var(--space-6);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 9rem), 1fr));
  gap: var(--space-3) var(--space-5);
  margin: 0;
}

.facts dt {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
}

.facts dd {
  margin: 0;
  font-size: var(--text-lg);
  font-variant-numeric: tabular-nums;
}

.lede {
  margin: 0;
  font-size: var(--text-sm);
}

.lede strong {
  color: var(--text);
  font-variant-numeric: tabular-nums;
}

.bands {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.bands li {
  display: grid;
  grid-template-columns: 5.5rem 1fr 4rem;
  align-items: center;
  gap: var(--space-3);
  font-size: var(--text-sm);
}

.band-name {
  font-variant-numeric: tabular-nums;
  color: var(--text-muted);
}

.band-count {
  text-align: right;
}

.extremes {
  gap: var(--space-6);
  font-size: var(--text-sm);
  flex-wrap: wrap;
}

.extremes p {
  margin: 0;
}

.tabular {
  font-variant-numeric: tabular-nums;
}

.charts {
  display: grid;
  gap: var(--space-6) var(--space-6);
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 17rem), 1fr));
}

.kinds {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.kinds li {
  display: grid;
  grid-template-columns: 8rem 1fr 4rem;
  align-items: center;
  gap: var(--space-3);
  font-size: var(--text-sm);
}

.meter {
  height: 0.5rem;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 8%, transparent);
  overflow: hidden;
}

.fill {
  display: block;
  height: 100%;
  border-radius: var(--radius-pill);
  background: var(--accent);
}

.kind-count {
  text-align: right;
}

.controls {
  gap: var(--space-2);
}

.word-search {
  flex: 1 1 14rem;
}

.word-sort {
  min-width: 11rem;
}

.controls :deep(.p-inputtext),
.controls :deep(.p-select) {
  height: 2.75rem;
}

.controls :deep(.p-select-label) {
  display: flex;
  align-items: center;
}

.count {
  font-size: var(--text-sm);
  margin: 0;
}

.words {
  list-style: none;
  margin: 0;
  padding: 0;
  columns: 19rem;
  column-gap: var(--space-4);
}

.words li {
  break-inside: avoid;
  margin-bottom: var(--space-2);
}

.word {
  width: 100%;
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: var(--text-sm);
  text-align: left;
  cursor: pointer;
  transition:
    border-color 0.15s ease,
    background-color 0.15s ease;
}

.word:hover {
  background: color-mix(in srgb, var(--text) 6%, transparent);
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
}

.word .text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 0 1 auto;
}

.word .leader {
  flex: 1 1 1rem;
  min-width: 0.75rem;
  align-self: center;
  border-bottom: 1px dotted color-mix(in srgb, var(--text) 30%, transparent);
}

.word .uses {
  font-weight: 600;
  flex: none;
}

.word .entries {
  font-size: var(--text-xs);
  flex: none;
}

.lines {
  gap: var(--space-2);
}

.empty {
  padding: var(--space-6) 0;
  text-align: center;
}

code {
  background: color-mix(in srgb, var(--text) 8%, transparent);
  border-radius: var(--radius-sm);
  padding: var(--space-0) var(--space-1);
  font-size: 0.9em;
}

@media (max-width: 34rem) {
  .kinds li,
  .bands li {
    grid-template-columns: 5rem 1fr 3.5rem;
    gap: var(--space-2);
    font-size: var(--text-sm);
  }
}
</style>
