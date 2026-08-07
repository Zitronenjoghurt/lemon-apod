<script lang="ts" setup>
import { computed, onMounted, ref } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import RetryNotice from '@/components/RetryNotice.vue'
import WordDetail from '@/components/WordDetail.vue'
import YearChart from '@/components/YearChart.vue'
import { api } from '@/api/client'
import type { SortOrder, Word, WordSort } from '@/api/types'
import { useAsync } from '@/composables/useAsync'
import { formatDate } from '@/utils/date'

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

/// The open word lives in the URL so a particular word's history can be linked to.
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

/// Zero until the archive has been reparsed by a build that records word counts, which is a
/// state worth naming rather than showing a page of zeroes.
const unmeasured = computed(() => stats.value != null && stats.value.text.measured === 0)

const years = computed(() => timeline.value?.years ?? [])

function series(pick: (year: (typeof years.value)[number]) => number) {
  return years.value.map((year) => ({ year: year.year, value: pick(year) }))
}

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
  if (value === undefined) return '—'
  return value.toLocaleString(undefined, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  })
}

function count(value: number | undefined): string {
  return value === undefined ? '—' : value.toLocaleString()
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
        <span class="muted name">Resources linked</span>
        <strong class="value">{{ count(catalogue?.resources) }}</strong>
        <span class="muted foot">
          across {{ count(catalogue?.hosts) }} sites,
          <RouterLink to="/resources">browse them</RouterLink>
        </span>
      </div>
      <div class="card tile">
        <span class="muted name">Under copyright</span>
        <strong class="value">{{ count(stats.copyright) }}</strong>
        <span class="muted foot">
          {{ round((stats.copyright / Math.max(stats.entries, 1)) * 100, 0) }}% of entries
        </span>
      </div>
    </section>

    <section v-if="text && text.measured" class="card panel">
      <h2>A typical explanation</h2>
      <dl class="facts">
        <div>
          <dt>Words</dt>
          <dd>{{ round(text.avg_words) }}</dd>
        </div>
        <div>
          <dt>Median</dt>
          <dd>{{ count(text.median_words) }}</dd>
        </div>
        <div>
          <dt>Shortest</dt>
          <dd>{{ count(text.min_words) }}</dd>
        </div>
        <div>
          <dt>Longest</dt>
          <dd>{{ count(text.max_words) }}</dd>
        </div>
        <div>
          <dt>Different words</dt>
          <dd>{{ round(text.avg_unique_words) }}</dd>
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
          <dt>Links</dt>
          <dd>{{ round(text.avg_links) }}</dd>
        </div>
      </dl>

      <div class="row extremes">
        <p v-if="text.shortest" class="muted">
          Shortest:
          <RouterLink :to="`/${text.shortest.date}`">{{ text.shortest.title }}</RouterLink>
          <span class="tabular"> ({{ text.shortest.word_count }} words)</span>
        </p>
        <p v-if="text.longest" class="muted">
          Longest:
          <RouterLink :to="`/${text.longest.date}`">{{ text.longest.title }}</RouterLink>
          <span class="tabular"> ({{ text.longest.word_count }} words)</span>
        </p>
      </div>
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
        <YearChart :points="series((y) => y.entries)" label="Entries published" />
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
      <h2>What the archive is made of</h2>
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
        {{ count(words.total) }} {{ words.total === 1 ? 'word' : 'words' }} match
        <template v-if="text?.used_once && !query">
          &middot; {{ count(text.used_once) }} of them were used exactly once
        </template>
      </p>

      <div v-if="wordsLoading && !words" class="stack lines">
        <Skeleton v-for="index in 6" :key="index" height="1.6rem" width="100%" />
      </div>

      <p v-else-if="words && !words.items.length" class="muted empty">
        No word in the archive matches that.
      </p>

      <ul v-else-if="words" class="words">
        <li v-for="word in words.items" :key="word.word">
          <button class="word" type="button" @click="openDetail(word)">
            <span class="text">{{ word.word }}</span>
            <span class="tabular uses">{{ count(word.total) }}&times;</span>
            <span class="muted tabular entries">in {{ count(word.entries) }}</span>
          </button>
        </li>
      </ul>

      <Paginator
        v-if="words && words.total > PAGE_SIZE"
        :first="(page - 1) * PAGE_SIZE"
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
  gap: 1.5rem;
}

h1 {
  font-size: 1.6rem;
}

h2 {
  font-size: 1.1rem;
  margin-bottom: 0.35rem;
}

.head {
  gap: 0.4rem;
}

.note {
  margin: 0;
  font-size: 0.88rem;
  max-width: 64ch;
  text-wrap: pretty;
}

.tiles {
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 13rem), 1fr));
}

.tile {
  padding: 1rem 1.1rem;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}

.tile .name {
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.tile .value {
  font-size: 1.7rem;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.02em;
  line-height: 1.2;
}

.tile .foot {
  font-size: 0.8rem;
}

.panel {
  padding: 1.2rem 1.3rem 1.4rem;
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 9rem), 1fr));
  gap: 0.8rem 1.2rem;
  margin: 0;
}

.facts dt {
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
}

.facts dd {
  margin: 0;
  font-size: 1.25rem;
  font-variant-numeric: tabular-nums;
}

.extremes {
  gap: 1.5rem;
  font-size: 0.88rem;
}

.extremes p {
  margin: 0;
}

.tabular {
  font-variant-numeric: tabular-nums;
}

.charts {
  display: grid;
  gap: 1.4rem 1.6rem;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 17rem), 1fr));
}

.kinds {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
}

.kinds li {
  display: grid;
  grid-template-columns: 8rem 1fr 4rem;
  align-items: center;
  gap: 0.75rem;
  font-size: 0.88rem;
}

.meter {
  height: 0.5rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 8%, transparent);
  overflow: hidden;
}

.fill {
  display: block;
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
}

.kind-count {
  text-align: right;
}

.controls {
  gap: 0.6rem;
}

.word-search {
  flex: 1 1 14rem;
}

.word-sort {
  min-width: 11rem;
}

.count {
  font-size: 0.85rem;
  margin: 0;
}

.words {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 0.25rem;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 15rem), 1fr));
}

.word {
  width: 100%;
  display: grid;
  grid-template-columns: 1fr auto auto;
  align-items: baseline;
  gap: 0.5rem;
  padding: 0.3rem 0.5rem;
  border: 0;
  border-radius: 0.45rem;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 0.9rem;
  text-align: left;
  cursor: pointer;
}

.word:hover {
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.word .text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.word .uses {
  font-weight: 600;
}

.word .entries {
  font-size: 0.78rem;
}

.lines {
  gap: 0.4rem;
}

.empty {
  padding: 1.5rem 0;
  text-align: center;
}

code {
  background: color-mix(in srgb, var(--text) 8%, transparent);
  border-radius: 0.3rem;
  padding: 0.05rem 0.3rem;
  font-size: 0.9em;
}

@media (max-width: 34rem) {
  .kinds li {
    grid-template-columns: 6.5rem 1fr 3.5rem;
    gap: 0.5rem;
    font-size: 0.82rem;
  }
}
</style>
