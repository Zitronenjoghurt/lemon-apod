<script lang="ts" setup>
import { computed, onMounted, ref, useTemplateRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import EntryGrid from '@/components/EntryGrid.vue'
import ReadFilter from '@/components/ReadFilter.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import type { KindFilter } from '@/api/types'
import { useAsync } from '@/composables/useAsync'
import { useNarrow } from '@/composables/useNarrow'
import { provideReadScope, useRead } from '@/composables/useRead'
import { FIRST_ENTRY } from '@/utils/date'

const route = useRoute()
const router = useRouter()
const { pageLinks } = useNarrow()

const PAGE_SIZE = 30
const DEBOUNCE_MS = 250

const KINDS: { group: string; kinds: { label: string; value: KindFilter }[] }[] = [
  {
    group: 'Images',
    kinds: [
      { label: 'Any image', value: 'image' },
      { label: 'JPEG', value: 'image_jpg' },
      { label: 'PNG', value: 'image_png' },
      { label: 'GIF', value: 'image_gif' },
      { label: 'TIFF', value: 'image_tiff' },
    ],
  },
  {
    group: 'Video',
    kinds: [
      { label: 'Any video', value: 'video' },
      { label: 'MP4 file', value: 'video_mp4' },
      { label: 'YouTube', value: 'youtube' },
      { label: 'Vimeo', value: 'vimeo' },
    ],
  },
  { group: 'Other', kinds: [{ label: 'Interactive page', value: 'embed' }] },
]

const KIND_LABELS: Record<string, string> = Object.fromEntries(
  KINDS.flatMap((group) => group.kinds).map((kind) => [kind.value, kind.label]),
)

const SORTS: { label: string; value: 'relevance' | 'date' }[] = [
  { label: 'Relevance', value: 'relevance' },
  { label: 'Newest', value: 'date' },
]

const RIGHTS: { label: string; value: 'any' | 'yes' | 'no' }[] = [
  { label: 'Any', value: 'any' },
  { label: 'Copyrighted', value: 'yes' },
  { label: 'No claim', value: 'no' },
]

const CONDITION: { label: string; value: 'any' | 'existing' | 'lost' }[] = [
  { label: 'Any', value: 'any' },
  { label: 'Existing', value: 'existing' },
  { label: 'Lost', value: 'lost' },
]

const SYNTAX: { example: string; means: string }[] = [
  { example: 'crab nebula', means: 'both words, anywhere in the entry' },
  { example: '"star cluster"', means: 'the words next to each other, in that order' },
  { example: 'galaxy -hubble', means: 'galaxy, but not hubble' },
  { example: 'comet OR asteroid', means: 'either word, rather than both. Write OR in uppercase' },
  { example: 'neb*', means: 'any word starting with neb' },
]

const FIRST_YEAR = Number(FIRST_ENTRY.slice(0, 4))
const YEARS = Array.from(
  { length: new Date().getUTCFullYear() - FIRST_YEAR + 1 },
  (_, step) => FIRST_YEAR + step,
).reverse()

function yearOf(raw: unknown): number | null {
  const found = Number.parseInt(String(raw ?? '').slice(0, 4), 10)
  return YEARS.includes(found) ? found : null
}

function kindsOf(raw: unknown): KindFilter[] {
  const known = new Set(Object.keys(KIND_LABELS))
  return String(raw ?? '')
    .split(',')
    .filter((one): one is KindFilter => known.has(one))
}

const query = ref(String(route.query.q ?? ''))
const kinds = ref<KindFilter[]>(kindsOf(route.query.kind))
const sort = ref<'relevance' | 'date'>(route.query.sort === 'date' ? 'date' : 'relevance')
const from = ref<number | null>(yearOf(route.query.from))
const to = ref<number | null>(yearOf(route.query.to))
const rights = ref<'any' | 'yes' | 'no'>(
  route.query.copyright === 'true' ? 'yes' : route.query.copyright === 'false' ? 'no' : 'any',
)
const media = ref<'any' | 'existing' | 'lost'>(
  route.query.lost === 'true' ? 'lost' : route.query.lost === 'false' ? 'existing' : 'any',
)
const page = ref(Number.parseInt(String(route.query.page ?? '1'), 10) || 1)

const panelOpen = ref(false)
const help = useTemplateRef<{ toggle: (event: Event) => void }>('help')
provideReadScope('search')
const { apply, active: filtered } = useRead('search')

const typed = computed(() => query.value.trim().length > 0)
const copyright = computed(() => (rights.value === 'any' ? undefined : rights.value === 'yes'))
const lost = computed(() => (media.value === 'any' ? undefined : media.value === 'lost'))

const narrowed = computed(() => {
  const chips: { key: string; label: string; drop: () => void }[] = []

  for (const kind of kinds.value) {
    chips.push({
      key: `kind:${kind}`,
      label: KIND_LABELS[kind] ?? kind,
      drop: () => (kinds.value = kinds.value.filter((one) => one !== kind)),
    })
  }
  if (from.value) {
    chips.push({ key: 'from', label: `From ${from.value}`, drop: () => (from.value = null) })
  }
  if (to.value) {
    chips.push({ key: 'to', label: `To ${to.value}`, drop: () => (to.value = null) })
  }
  if (rights.value !== 'any') {
    chips.push({
      key: 'rights',
      label: rights.value === 'yes' ? 'Copyrighted' : 'No copyright claim',
      drop: () => (rights.value = 'any'),
    })
  }
  if (media.value !== 'any') {
    chips.push({
      key: 'media',
      label: media.value === 'lost' ? 'Lost media' : 'Existing media',
      drop: () => (media.value = 'any'),
    })
  }

  return chips
})

/** Filters on their own are a search. Only an untouched page has nothing to show. */
const asked = computed(() => typed.value || narrowed.value.length > 0)

const {
  data: results,
  error,
  loading,
  run,
} = useAsync((signal) =>
  api.search(
    query.value,
    {
      kind: kinds.value.length ? (kinds.value.join(',') as KindFilter) : undefined,
      from: from.value ? `${from.value}-01-01` : undefined,
      to: to.value ? `${to.value}-12-31` : undefined,
      copyright: copyright.value,
      lost: lost.value,
      sort: sort.value,
      offset: (page.value - 1) * PAGE_SIZE,
      limit: PAGE_SIZE,
    },
    signal,
  ),
)

const shown = computed(() => apply(results.value?.items ?? []))
const hidden = computed(() => (results.value?.items.length ?? 0) - shown.value.length)

let debounce: ReturnType<typeof setTimeout> | undefined

function search(resetPage: boolean) {
  if (resetPage) page.value = 1

  clearTimeout(debounce)
  debounce = setTimeout(() => {
    router.replace({
      name: 'search',
      query: {
        q: query.value || undefined,
        kind: kinds.value.length ? kinds.value.join(',') : undefined,
        from: from.value ? `${from.value}-01-01` : undefined,
        to: to.value ? `${to.value}-12-31` : undefined,
        copyright: copyright.value === undefined ? undefined : String(copyright.value),
        lost: lost.value === undefined ? undefined : String(lost.value),
        sort: sort.value === 'relevance' ? undefined : sort.value,
        page: page.value === 1 ? undefined : String(page.value),
      },
    })

    if (asked.value) run()
  }, DEBOUNCE_MS)
}

function drop(chip: { drop: () => void }) {
  chip.drop()
  search(true)
}

function chooseSort(value: 'relevance' | 'date' | null) {
  sort.value = value ?? 'relevance'
  search(true)
}

function chooseRights(value: 'any' | 'yes' | 'no' | null) {
  rights.value = value ?? 'any'
  search(true)
}

function chooseCondition(value: 'any' | 'existing' | 'lost' | null) {
  media.value = value ?? 'any'
  search(true)
}

function clear() {
  kinds.value = []
  from.value = null
  to.value = null
  rights.value = 'any'
  media.value = 'any'
  search(true)
}

function onPage(event: { page: number }) {
  page.value = event.page + 1
  search(false)
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

watch(
  () => route.query.q,
  (next) => {
    const incoming = String(next ?? '')
    if (incoming !== query.value) {
      query.value = incoming
      search(true)
    }
  },
)

onMounted(() => {
  if (asked.value) run()
})

const onlyExclusions = computed(
  () =>
    typed.value &&
    results.value?.total === 0 &&
    query.value
      .trim()
      .split(/\s+/)
      .every((token) => token.startsWith('-') || token === 'OR' || token === 'NOT'),
)
</script>

<template>
  <div class="stack">
    <div class="row search-bar">
      <IconField class="search-field">
        <InputIcon class="pi pi-search" />
        <InputText
          v-model="query"
          aria-label="Search entries"
          autofocus
          fluid
          placeholder="Search..."
          size="large"
          type="search"
          @input="search(true)"
        />
      </IconField>

      <Button
        :aria-expanded="panelOpen"
        :badge="narrowed.length ? String(narrowed.length) : undefined"
        :outlined="!panelOpen"
        badge-severity="contrast"
        class="filters-button"
        icon="pi pi-sliders-h"
        label="Filters"
        severity="secondary"
        size="large"
        @click="panelOpen = !panelOpen"
      />

      <Button
        aria-label="Search syntax"
        icon="pi pi-question-circle"
        rounded
        severity="secondary"
        size="large"
        text
        @click="help?.toggle($event)"
      />
    </div>

    <div v-if="panelOpen" class="card panel">
      <div class="control">
        <label class="muted name" for="kinds">Kind</label>
        <MultiSelect
          id="kinds"
          v-model="kinds"
          :max-selected-labels="2"
          :options="KINDS"
          :show-toggle-all="false"
          option-group-children="kinds"
          option-group-label="group"
          option-label="label"
          option-value="value"
          placeholder="Any kind"
          selected-items-label="{0} kinds"
          size="small"
          @update:model-value="search(true)"
        />
      </div>

      <div class="control">
        <span id="sort-label" class="muted name">Order</span>
        <SelectButton
          v-tooltip.bottom="{
            value: 'Relevance needs a search text to be relevant to',
            disabled: typed,
          }"
          :allow-empty="false"
          :disabled="!typed"
          :model-value="typed ? sort : 'date'"
          :options="SORTS"
          aria-labelledby="sort-label"
          option-label="label"
          option-value="value"
          size="small"
          @update:model-value="chooseSort"
        />
      </div>

      <div class="control">
        <span class="muted name">Years</span>
        <div class="row span">
          <Select
            v-model="from"
            :options="YEARS"
            aria-label="From year"
            placeholder="From"
            show-clear
            size="small"
            @update:model-value="search(true)"
          />
          <Select
            v-model="to"
            :options="YEARS"
            aria-label="To year"
            placeholder="To"
            show-clear
            size="small"
            @update:model-value="search(true)"
          />
        </div>
      </div>

      <div class="control">
        <span id="rights-label" class="muted name">Credit</span>
        <SelectButton
          :allow-empty="false"
          :model-value="rights"
          :options="RIGHTS"
          aria-labelledby="rights-label"
          option-label="label"
          option-value="value"
          size="small"
          @update:model-value="chooseRights"
        />
      </div>

      <div class="control">
        <span id="media-label" class="muted name">Media</span>
        <SelectButton
          :allow-empty="false"
          :model-value="media"
          :options="CONDITION"
          aria-labelledby="media-label"
          option-label="label"
          option-value="value"
          size="small"
          @update:model-value="chooseCondition"
        />
      </div>

      <Button
        v-if="narrowed.length"
        class="clear"
        label="Clear filters"
        severity="secondary"
        size="small"
        text
        @click="clear"
      />
    </div>

    <ul v-if="narrowed.length" aria-label="Filters in force" class="row chips">
      <li v-for="chip in narrowed" :key="chip.key">
        <button :aria-label="`Remove filter: ${chip.label}`" type="button" @click="drop(chip)">
          {{ chip.label }}
          <i aria-hidden="true" class="pi pi-times" />
        </button>
      </li>
    </ul>

    <Popover ref="help">
      <dl class="syntax">
        <template v-for="row in SYNTAX" :key="row.example">
          <dt>
            <code>{{ row.example }}</code>
          </dt>
          <dd class="muted">{{ row.means }}</dd>
        </template>
      </dl>
    </Popover>

    <div v-if="asked" class="row summary">
      <p v-if="results" aria-live="polite" class="muted count">
        {{ results.total.toLocaleString() }}
        {{ results.total === 1 ? 'entry' : 'entries' }}
      </p>
      <ReadFilter :hidden="hidden" class="read" />
    </div>

    <RetryNotice v-if="error" :busy="loading" :message="error" @retry="run" />

    <p v-if="!asked" class="muted empty">
      Search titles, explanations, credits and keywords, or filter the archive without typing
      anything.
    </p>

    <Message v-else-if="onlyExclusions" :closable="false" severity="secondary">
      A search made only of exclusions has nothing to match. Add a word to look for.
    </Message>

    <EntryGrid
      v-else-if="!error || shown.length"
      :empty="
        filtered && hidden
          ? 'Every result on this page is filtered out by the read filter.'
          : 'No entry matches that.'
      "
      :entries="shown"
      :loading="loading"
      :query="query"
    />

    <Paginator
      v-if="results && results.total > PAGE_SIZE"
      :first="(page - 1) * PAGE_SIZE"
      :page-link-size="pageLinks"
      :rows="PAGE_SIZE"
      :total-records="results.total"
      @page="onPage"
    />
  </div>
</template>

<style scoped>
.search-bar {
  gap: var(--space-2);
  align-items: stretch;
  flex-wrap: nowrap;
}

.search-field {
  flex: 1 1 auto;
  min-width: 0;
}

.search-field :deep(input) {
  height: 100%;
}

.search-bar :deep(.p-button) {
  height: auto;
}

.panel {
  display: flex;
  flex-wrap: wrap;
  align-items: start;
  gap: var(--space-4) var(--space-6);
  padding: var(--space-4);
}

.control {
  display: grid;
  grid-template-rows: auto 2.375rem;
  align-items: center;
  flex: 0 0 auto;
  gap: var(--space-2);
}

.control :deep(.p-multiselect) {
  min-width: 13rem;
}

.name {
  align-self: end;
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  line-height: 1.4;
}

.span {
  gap: var(--space-2);
}

.span :deep(.p-select) {
  min-width: 7rem;
}

.control :deep(.p-togglebutton-label),
.control :deep(.p-selectbutton) {
  white-space: nowrap;
}

.clear {
  margin-left: auto;
  align-self: end;
}

.chips {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.chips button {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.15rem 0.6rem;
  border: 1px solid color-mix(in srgb, var(--accent) 45%, var(--border));
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--accent) 10%, transparent);
  color: inherit;
  font: inherit;
  font-size: var(--text-xs);
  cursor: pointer;
}

.chips button:hover {
  border-color: var(--accent);
}

.chips i {
  font-size: 0.7em;
  opacity: 0.8;
}

.summary {
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.count {
  font-size: 0.88rem;
  margin: 0;
}

.read {
  margin-left: auto;
}

.syntax {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 0.4rem 0.9rem;
  margin: 0;
  max-width: 22rem;
  font-size: 0.85rem;
}

.syntax dt code {
  background: color-mix(in srgb, var(--text) 8%, transparent);
  border-radius: 0.3rem;
  padding: 0.1rem 0.35rem;
  white-space: nowrap;
}

.syntax dd {
  margin: 0;
  text-wrap: pretty;
}

.empty {
  padding: 3rem 0;
  text-align: center;
  text-wrap: pretty;
}

@media (max-width: 30rem) {
  .filters-button :deep(.p-button-label) {
    display: none;
  }

  .panel {
    flex-direction: column;
    align-items: stretch;
  }

  .clear {
    margin-left: 0;
  }
}
</style>
