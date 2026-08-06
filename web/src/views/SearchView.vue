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

const KINDS: { label: string; value: MediaKind | '' }[] = [
  { label: 'Anything', value: '' },
  { label: 'Images', value: 'image_jpg' },
  { label: 'Video', value: 'youtube' },
]

const query = ref(String(route.query.q ?? ''))
const kind = ref<MediaKind | ''>((route.query.kind as MediaKind) ?? '')
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
      kind: kind.value || undefined,
      sort: sort.value,
      offset: (page.value - 1) * PAGE_SIZE,
      limit: PAGE_SIZE,
    },
    signal,
  ),
)

const totalPages = computed(() =>
  results.value ? Math.max(1, Math.ceil(results.value.total / PAGE_SIZE)) : 1,
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
        kind: kind.value || undefined,
        sort: sort.value === 'relevance' ? undefined : sort.value,
        page: page.value === 1 ? undefined : String(page.value),
      },
    })

    if (query.value.trim()) run()
  }, DEBOUNCE_MS)
}

function selectKind(value: MediaKind | '') {
  kind.value = value
  search(true)
}

function selectSort(value: 'relevance' | 'date') {
  sort.value = value
  search(true)
}

function goTo(target: number) {
  page.value = Math.min(Math.max(1, target), totalPages.value)
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
</script>

<template>
  <div class="stack">
    <div class="search-bar card">
      <i class="pi pi-search" aria-hidden="true" />
      <input
        v-model="query"
        type="search"
        placeholder="Search 30 years of explanations, titles and credits…"
        aria-label="Search entries"
        autofocus
        @input="search(true)"
      />
    </div>

    <div class="row filters">
      <div class="row group" role="group" aria-label="Media kind">
        <button
          v-for="option in KINDS"
          :key="option.value"
          type="button"
          class="chip"
          :class="{ active: kind === option.value }"
          @click="selectKind(option.value)"
        >
          {{ option.label }}
        </button>
      </div>

      <div class="row group" role="group" aria-label="Sort order">
        <button
          type="button"
          class="chip"
          :class="{ active: sort === 'relevance' }"
          @click="selectSort('relevance')"
        >
          Relevance
        </button>
        <button
          type="button"
          class="chip"
          :class="{ active: sort === 'date' }"
          @click="selectSort('date')"
        >
          Newest
        </button>
      </div>
    </div>

    <p v-if="results && query.trim()" class="muted count" aria-live="polite">
      {{ results.total.toLocaleString() }}
      {{ results.total === 1 ? 'result' : 'results' }}
    </p>

    <p v-if="error" class="muted">{{ error }}</p>

    <p v-else-if="!query.trim()" class="muted empty">
      Type to search titles, explanations, credits and keywords.
    </p>

    <EntryGrid
      v-else
      :entries="results?.items"
      :loading="loading"
      empty="No entries matched that search."
    />

    <nav v-if="results && totalPages > 1" class="row pager" aria-label="Pagination">
      <button type="button" class="chip" :disabled="page <= 1" @click="goTo(page - 1)">
        <i class="pi pi-chevron-left" aria-hidden="true" /> Previous
      </button>
      <span class="muted">Page {{ page }} of {{ totalPages }}</span>
      <button type="button" class="chip" :disabled="page >= totalPages" @click="goTo(page + 1)">
        Next <i class="pi pi-chevron-right" aria-hidden="true" />
      </button>
    </nav>
  </div>
</template>

<style scoped>
.search-bar {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.85rem 1.1rem;
}

.search-bar i {
  color: var(--text-muted);
}

.search-bar input {
  flex: 1;
  border: 0;
  background: none;
  font: inherit;
  font-size: 1.05rem;
  color: inherit;
  outline: none;
  min-width: 0;
}

.filters {
  justify-content: space-between;
  gap: 1rem;
}

.group {
  gap: 0.35rem;
}

.chip {
  font: inherit;
  font-size: 0.86rem;
  padding: 0.3rem 0.8rem;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--bg-elevated);
  color: var(--text-muted);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
}

.chip:hover:not(:disabled) {
  color: var(--text);
}

.chip.active {
  color: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  background: color-mix(in srgb, var(--accent) 10%, var(--bg-elevated));
}

.chip:disabled {
  opacity: 0.45;
  cursor: default;
}

.count {
  font-size: 0.88rem;
  margin: 0;
}

.empty {
  padding: 3rem 0;
  text-align: center;
}

.pager {
  justify-content: center;
  gap: 1rem;
  padding-top: 1rem;
}
</style>
