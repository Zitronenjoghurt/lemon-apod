<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import EntryGrid from '@/components/EntryGrid.vue'
import { api } from '@/api/client'
import type { MediaKind } from '@/api/types'
import { useAsync } from '@/composables/useAsync'

const route = useRoute()
const router = useRouter()

const PAGE_SIZE = 30
const DEBOUNCE_MS = 250

const ANY = 'any' as const
type KindChoice = MediaKind | typeof ANY

const KINDS: { label: string; value: KindChoice }[] = [
  { label: 'Anything', value: ANY },
  { label: 'Images', value: 'image_jpg' },
  { label: 'Video', value: 'video_mp4' },
]

const SORTS: { label: string; value: 'relevance' | 'date' }[] = [
  { label: 'Relevance', value: 'relevance' },
  { label: 'Newest', value: 'date' },
]

const query = ref(String(route.query.q ?? ''))
const kind = ref<KindChoice>((route.query.kind as MediaKind) ?? ANY)
const sort = ref<'relevance' | 'date'>(route.query.sort === 'date' ? 'date' : 'relevance')
const page = ref(Number.parseInt(String(route.query.page ?? '1'), 10) || 1)

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
</script>

<template>
  <div class="stack">
    <IconField class="search-field">
      <InputIcon class="pi pi-search" />
      <InputText
        v-model="query"
        type="search"
        placeholder="Search 30 years of explanations, titles and credits…"
        aria-label="Search entries"
        autofocus
        fluid
        size="large"
        @input="search(true)"
      />
    </IconField>

    <div class="filters">
      <SelectButton
        :model-value="kind"
        :options="KINDS"
        option-label="label"
        option-value="value"
        size="small"
        aria-labelledby="kind-label"
        @update:model-value="selectKind"
      />
      <span id="kind-label" class="sr-only">Media kind</span>

      <SelectButton
        :model-value="sort"
        :options="SORTS"
        option-label="label"
        option-value="value"
        size="small"
        aria-labelledby="sort-label"
        @update:model-value="selectSort"
      />
      <span id="sort-label" class="sr-only">Sort order</span>
    </div>

    <p v-if="results && hasQuery" class="muted count" aria-live="polite">
      {{ results.total.toLocaleString() }}
      {{ results.total === 1 ? 'result' : 'results' }}
    </p>

    <Message v-if="error" severity="error" :closable="false">{{ error }}</Message>

    <p v-else-if="!hasQuery" class="muted empty">
      Type to search titles, explanations, credits and keywords.
    </p>

    <EntryGrid
      v-else
      :entries="results?.items"
      :loading="loading"
      empty="No entries matched that search."
    />

    <Paginator
      v-if="results && results.total > PAGE_SIZE"
      :rows="PAGE_SIZE"
      :total-records="results.total"
      :first="(page - 1) * PAGE_SIZE"
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

.count {
  font-size: 0.88rem;
  margin: 0;
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
