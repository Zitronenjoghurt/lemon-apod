<script lang="ts" setup>
import { computed, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import type { HostCount, ResourceSort, SortOrder } from '@/api/types'
import { useAsync } from '@/composables/useAsync'
import { useNarrow } from '@/composables/useNarrow'
import { year as yearOf } from '@/utils/date'

const PAGE_SIZE = 30
const DEBOUNCE_MS = 250

const SORTS: { label: string; value: `${ResourceSort}:${SortOrder}` }[] = [
  { label: 'Most referenced', value: 'refs:desc' },
  { label: 'Least referenced', value: 'refs:asc' },
  { label: 'Newest reference', value: 'last:desc' },
  { label: 'First referenced', value: 'first:asc' },
  { label: 'Name, A to Z', value: 'label:asc' },
  { label: 'Name, Z to A', value: 'label:desc' },
  { label: 'By address', value: 'address:asc' },
]

const route = useRoute()
const router = useRouter()
const { pageLinks } = useNarrow()

const query = ref(String(route.query.q ?? ''))
const host = ref<string | null>((route.query.host as string) ?? null)
const sort = ref<`${ResourceSort}:${SortOrder}`>('refs:desc')
const page = ref(Number.parseInt(String(route.query.page ?? '1'), 10) || 1)

const hosts = ref<HostCount[]>([])

const {
  data: listing,
  error,
  loading,
  run,
} = useAsync((signal) => {
  const [by, order] = sort.value.split(':') as [ResourceSort, SortOrder]
  return api.resources(
    {
      q: query.value || undefined,
      host: host.value || undefined,
      sort: by,
      order,
      offset: (page.value - 1) * PAGE_SIZE,
      limit: PAGE_SIZE,
    },
    signal,
  )
})

let debounce: ReturnType<typeof setTimeout> | undefined

function search(resetPage: boolean) {
  if (resetPage) page.value = 1

  clearTimeout(debounce)
  debounce = setTimeout(() => {
    void router.replace({
      name: 'resources',
      query: {
        q: query.value || undefined,
        host: host.value || undefined,
        page: page.value === 1 ? undefined : String(page.value),
      },
    })
    void run()
  }, DEBOUNCE_MS)
}

function onPage(event: { page: number }) {
  page.value = event.page + 1
  search(false)
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

onMounted(() => {
  void run()
  api
    .resourceHosts()
    .then((found) => {
      hosts.value = found
    })
    .catch(() => {})
})

watch(
  () => route.query.host,
  (next) => {
    const incoming = (next as string) ?? null
    if (incoming !== host.value) {
      host.value = incoming
      search(true)
    }
  },
)

const hostOptions = computed(() =>
  hosts.value.map((entry) => ({
    label: `${entry.host} (${entry.resources.toLocaleString()})`,
    value: entry.host,
  })),
)

function nameOf(resource: { label?: string; key: string }): string {
  return resource.label?.trim() || resource.key
}

function span(first?: string, last?: string): string {
  if (!first || !last) return ''
  const from = yearOf(first)
  const to = yearOf(last)
  return from === to ? String(from) : `${from} to ${to}`
}
</script>

<template>
  <div class="stack">
    <header class="stack head">
      <h1>Resources</h1>
    </header>

    <div class="row controls">
      <IconField class="search">
        <InputIcon class="pi pi-search" />
        <InputText
          v-model="query"
          aria-label="Search resources"
          fluid
          placeholder="Search addresses and link text…"
          type="search"
          @input="search(true)"
        />
      </IconField>

      <Select
        v-model="host"
        :options="hostOptions"
        aria-label="Site"
        class="host"
        option-label="label"
        option-value="value"
        placeholder="Any site"
        show-clear
        @update:model-value="search(true)"
      />

      <Select
        v-model="sort"
        :options="SORTS"
        aria-label="Order"
        class="sort"
        option-label="label"
        option-value="value"
        @update:model-value="search(true)"
      />
    </div>

    <RetryNotice v-if="error" :busy="loading" :message="error" @retry="run" />

    <p v-if="listing" aria-live="polite" class="muted count">
      {{ listing.total.toLocaleString() }}
      {{ listing.total === 1 ? 'resource' : 'resources' }}
      <template v-if="host">on {{ host }}</template>
    </p>

    <div v-if="loading && !listing" class="stack lines">
      <Skeleton v-for="index in 8" :key="index" height="3.4rem" width="100%" />
    </div>

    <p v-else-if="listing && !listing.items.length" class="muted empty">
      Nothing in the catalogue matches that.
    </p>

    <ul v-else-if="listing" class="stack results">
      <li v-for="resource in listing.items" :key="resource.id" class="card item">
        <div class="body">
          <RouterLink :to="`/resources/${resource.id}`" class="name">
            {{ nameOf(resource) }}
          </RouterLink>
          <a :href="resource.url" class="muted address" rel="noopener nofollow" target="_blank">
            {{ resource.key }}
          </a>
        </div>

        <div class="meta">
          <span class="muted tabular years">{{ span(resource.first, resource.last) }}</span>
          <span
            :title="`${resource.refs} references across ${resource.entries} entries`"
            class="tabular refs"
          >
            {{ resource.refs.toLocaleString() }}
            <span class="muted unit">{{ resource.refs === 1 ? 'reference' : 'references' }}</span>
          </span>
        </div>
      </li>
    </ul>

    <Paginator
      :page-link-size="pageLinks"
      v-if="listing && listing.total > PAGE_SIZE"
      :first="(page - 1) * PAGE_SIZE"
      :rows="PAGE_SIZE"
      :total-records="listing.total"
      @page="onPage"
    />
  </div>
</template>

<style scoped>
h1 {
  font-size: 1.6rem;
}

.head {
  gap: 0.4rem;
}

.controls {
  gap: 0.6rem;
}

.search {
  flex: 1 1 16rem;
}

/* Keep the field the same height as the selects beside it. */
.controls :deep(.p-inputtext),
.controls :deep(.p-select) {
  height: 2.75rem;
}

.controls :deep(.p-select-label) {
  display: flex;
  align-items: center;
}

.host {
  min-width: 12rem;
}

.sort {
  min-width: 11rem;
}

.count {
  font-size: 0.85rem;
  margin: 0;
}

.results {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: 0.55rem;
}

.item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 0.9rem;
  padding: 0.7rem 1rem;
}

.body {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.name {
  font-weight: 600;
  text-decoration: none;
  color: inherit;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.name:hover {
  color: var(--accent);
}

.address {
  font-size: 0.8rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-decoration: none;
}

.address:hover {
  text-decoration: underline;
}

.meta {
  display: flex;
  align-items: baseline;
  gap: 1rem;
  text-align: right;
}

.tabular {
  font-variant-numeric: tabular-nums;
}

.years {
  font-size: 0.8rem;
}

.refs {
  font-weight: 600;
  font-size: 0.95rem;
}

.unit {
  font-weight: 400;
  font-size: 0.78rem;
}

.lines {
  gap: 0.55rem;
}

.empty {
  padding: 3rem 0;
  text-align: center;
}

@media (max-width: 44rem) {
  .years {
    display: none;
  }
}

@media (max-width: 30rem) {
  .item {
    padding: 0.6rem 0.8rem;
    gap: 0.7rem;
  }

  .unit {
    display: none;
  }
}
</style>
