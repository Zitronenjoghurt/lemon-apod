<script lang="ts" setup>
import { computed, onMounted, ref, useTemplateRef, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import EntryGrid from '@/components/EntryGrid.vue'
import ReadFilter from '@/components/ReadFilter.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import type { KindFilter } from '@/api/types'
import { useAsync } from '@/composables/useAsync'
import { useRead } from '@/composables/useRead'

const route = useRoute()
const router = useRouter()

const PAGE_SIZE = 30
const DEBOUNCE_MS = 250

const ANY = 'any' as const
type KindChoice = KindFilter | typeof ANY

const KINDS: { label: string; value: KindChoice }[] = [
  { label: 'Anything', value: ANY },
  { label: 'Images', value: 'image' },
  { label: 'Video', value: 'video' },
]

const SORTS: { label: string; value: 'relevance' | 'date' }[] = [
  { label: 'Relevance', value: 'relevance' },
  { label: 'Newest', value: 'date' },
]

const SYNTAX: { example: string; means: string }[] = [
  { example: 'crab nebula', means: 'both words, anywhere in the entry' },
  { example: '"star cluster"', means: 'the words next to each other, in that order' },
  { example: 'galaxy -hubble', means: 'galaxy, but not hubble' },
  { example: 'comet OR asteroid', means: 'either word, rather than both. Write OR in uppercase' },
  { example: 'neb*', means: 'any word starting with neb' },
]

const query = ref(String(route.query.q ?? ''))
const kind = ref<KindChoice>((route.query.kind as KindFilter) ?? ANY)
const sort = ref<'relevance' | 'date'>(route.query.sort === 'date' ? 'date' : 'relevance')
const page = ref(Number.parseInt(String(route.query.page ?? '1'), 10) || 1)

const help = useTemplateRef<{ toggle: (event: Event) => void }>('help')
const { apply, active: filtered } = useRead()

const {
  data: results,
  error,
  loading,
  run,
} = useAsync((signal) =>
  api.search(
    query.value,
    {
      kind: kind.value === ANY ? undefined : kind.value,
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
        kind: kind.value === ANY ? undefined : kind.value,
        sort: sort.value === 'relevance' ? undefined : sort.value,
        page: page.value === 1 ? undefined : String(page.value),
      },
    })

    if (query.value.trim()) run()
  }, DEBOUNCE_MS)
}

function selectKind(value: KindChoice | null) {
  kind.value = value ?? ANY
  search(true)
}

function selectSort(value: 'relevance' | 'date' | null) {
  sort.value = value ?? 'relevance'
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
  if (query.value.trim()) run()
})

const hasQuery = computed(() => query.value.trim().length > 0)

const onlyExclusions = computed(
  () =>
    hasQuery.value &&
    results.value?.total === 0 &&
    query.value
      .trim()
      .split(/\s+/)
      .every((token) => token.startsWith('-') || token === 'OR' || token === 'NOT'),
)
</script>

<template>
  <div class="stack">
    <IconField class="search-field">
      <InputIcon class="pi pi-search" />
      <InputText
        v-model="query"
        aria-label="Search entries"
        autofocus
        fluid
        placeholder="Search 30 years of explanations, titles and credits…"
        size="large"
        type="search"
        @input="search(true)"
      />
    </IconField>

    <div class="filters">
      <SelectButton
        :model-value="kind"
        :options="KINDS"
        aria-labelledby="kind-label"
        option-label="label"
        option-value="value"
        size="small"
        @update:model-value="selectKind"
      />
      <span id="kind-label" class="sr-only">Media kind</span>

      <div class="row trailing">
        <SelectButton
          :model-value="sort"
          :options="SORTS"
          aria-labelledby="sort-label"
          option-label="label"
          option-value="value"
          size="small"
          @update:model-value="selectSort"
        />
        <span id="sort-label" class="sr-only">Sort order</span>

        <Button
          aria-label="Search syntax"
          icon="pi pi-question-circle"
          rounded
          severity="secondary"
          size="small"
          text
          @click="help?.toggle($event)"
        />
      </div>
    </div>

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

    <div v-if="hasQuery" class="row summary">
      <p v-if="results" aria-live="polite" class="muted count">
        {{ results.total.toLocaleString() }}
        {{ results.total === 1 ? 'result' : 'results' }}
      </p>
      <ReadFilter :hidden="hidden" class="read" />
    </div>

    <RetryNotice v-if="error" :busy="loading" :message="error" @retry="run" />

    <p v-if="!hasQuery" class="muted empty">
      Type to search titles, explanations, credits and keywords.
    </p>

    <Message v-else-if="onlyExclusions" :closable="false" severity="secondary">
      A search made only of exclusions has nothing to match. Add a word to look for.
    </Message>

    <EntryGrid
      v-else-if="!error || shown.length"
      :empty="
        filtered && hidden
          ? 'Every result on this page is filtered out by the read filter.'
          : 'No entries matched that search.'
      "
      :entries="shown"
      :loading="loading"
      :query="query"
    />

    <Paginator
      v-if="results && results.total > PAGE_SIZE"
      :first="(page - 1) * PAGE_SIZE"
      :rows="PAGE_SIZE"
      :total-records="results.total"
      @page="onPage"
    />
  </div>
</template>

<style scoped>
.search-field {
  width: 100%;
}

.filters {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: 0.75rem;
}

.trailing {
  gap: 0.35rem;
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
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}

@media (max-width: 30rem) {
  .filters {
    flex-direction: column;
    align-items: stretch;
  }
}
</style>
