<script lang="ts" setup>
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import IndexRow from '@/components/IndexRow.vue'
import IndexTabs from '@/components/IndexTabs.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import type { HostCount, ResourceSort, SortOrder } from '@/api/types'
import { useIndexListing } from '@/composables/useIndexListing'
import { useNarrow } from '@/composables/useNarrow'
import { year as yearOf } from '@/utils/date'

const PAGE_SIZE = 30

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
const { pageLinks } = useNarrow()

const query = ref(String(route.query.q ?? ''))
const host = ref<string | null>((route.query.host as string) ?? null)
const sort = ref<`${ResourceSort}:${SortOrder}`>('refs:desc')

const hosts = ref<HostCount[]>([])

const {
  data: listing,
  error,
  loading,
  run,
  first,
  search,
  onPage,
} = useIndexListing({
  routeName: 'resources',
  pageSize: PAGE_SIZE,
  query: () => ({ q: query.value || undefined, host: host.value || undefined }),
  fetch: (offset, signal) => {
    const [by, order] = sort.value.split(':') as [ResourceSort, SortOrder]
    return api.resources(
      {
        q: query.value || undefined,
        host: host.value || undefined,
        sort: by,
        order,
        offset,
        limit: PAGE_SIZE,
      },
      signal,
    )
  },
})

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
    <IndexTabs />

    <div class="row controls">
      <IconField class="search">
        <InputIcon><AppIcon name="search" /></InputIcon>
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
    </p>

    <div v-if="loading && !listing" class="stack lines">
      <Skeleton v-for="index in 8" :key="index" height="3.4rem" width="100%" />
    </div>

    <p v-else-if="listing && !listing.items.length" class="muted empty">
      No resource matches that.
    </p>

    <ul v-else-if="listing" class="stack results">
      <IndexRow
        v-for="resource in listing.items"
        :key="resource.id"
        :name="nameOf(resource)"
        :to="`/resources/${resource.id}`"
      >
        <template #note>
          <a :href="resource.url" class="muted address" rel="noopener nofollow" target="_blank">
            {{ resource.key }}
          </a>
        </template>
        <template #meta>
          <span v-if="span(resource.first, resource.last)" class="muted years">
            {{ span(resource.first, resource.last) }}
          </span>
          <span
            :title="`${resource.refs} references across ${resource.entries} entries`"
            class="refs"
          >
            {{ resource.refs.toLocaleString() }}
            <span class="muted unit">{{ resource.refs === 1 ? 'reference' : 'references' }}</span>
          </span>
        </template>
      </IndexRow>
    </ul>

    <Paginator
      v-if="listing && listing.total > PAGE_SIZE"
      :first="first()"
      :page-link-size="pageLinks"
      :rows="PAGE_SIZE"
      :total-records="listing.total"
      @page="onPage"
    />
  </div>
</template>

<style scoped>
h1 {
  font-size: var(--text-xl);
}

.controls {
  gap: var(--space-2);
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
  min-width: 12rem;
}

.count {
  margin: calc(var(--space-2) - var(--gap)) 0 0;
  font-size: var(--text-sm);
}

.results {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: var(--space-2);
}

.address {
  font-size: var(--text-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-decoration: none;
}

.address:hover {
  text-decoration: underline;
}

.years {
  font-size: var(--text-sm);
}

.refs {
  font-weight: 600;
  font-size: var(--text-md);
}

.unit {
  font-weight: 400;
  font-size: var(--text-xs);
}

.lines {
  gap: var(--space-2);
}

.empty {
  padding: var(--space-8) 0;
  text-align: center;
}

@media (max-width: 44rem) {
  .years {
    display: none;
  }
}

@media (max-width: 30rem) {
  .unit {
    display: none;
  }

  .host,
  .sort {
    width: 100%;
  }
}
</style>
