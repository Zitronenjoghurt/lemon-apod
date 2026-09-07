<script lang="ts" setup>
import { computed, onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import IndexRow from '@/components/IndexRow.vue'
import IndexTabs from '@/components/IndexTabs.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import type { CatalogCount, ObjectSort } from '@/api/types'
import { useIndexListing } from '@/composables/useIndexListing'
import { useNarrow } from '@/composables/useNarrow'
import { year as yearOf } from '@/utils/date'

const PAGE_SIZE = 30

const SORTS: { label: string; value: ObjectSort }[] = [
  { label: 'Most shown', value: 'entries' },
  { label: 'Shown most recently', value: 'latest' },
  { label: 'By designation', value: 'designation' },
]

const CATALOGS: Record<string, string> = {
  messier: 'Messier',
  ngc: 'NGC',
  ic: 'IC',
  sharpless: 'Sharpless',
  abell: 'Abell',
  caldwell: 'Caldwell',
  barnard: 'Barnard',
  arp: 'Arp',
  ldn: 'Lynds dark',
  lbn: 'Lynds bright',
  vdb: 'van den Bergh',
  cederblad: 'Cederblad',
  melotte: 'Melotte',
  collinder: 'Collinder',
  trumpler: 'Trumpler',
  hickson: 'Hickson',
  terzan: 'Terzan',
  palomar: 'Palomar',
  ugc: 'UGC',
  pgc: 'PGC',
  rcw: 'RCW',
  gum: 'Gum',
  simeis: 'Simeis',
  'herbig-haro': 'Herbig-Haro',
  supernova: 'Supernovae',
  comet: 'Comets',
  solar: 'The solar system',
  star: 'Stars',
  shower: 'Meteor showers',
  constellation: 'Constellations',
  named: 'Named without a catalogue',
}

const route = useRoute()
const { pageLinks } = useNarrow()

const query = ref(String(route.query.q ?? ''))
const catalog = ref<string | null>((route.query.catalog as string) ?? null)
const sort = ref<ObjectSort>(
  SORTS.some((option) => option.value === route.query.sort)
    ? (route.query.sort as ObjectSort)
    : 'entries',
)

const catalogs = ref<CatalogCount[]>([])

const {
  data: listing,
  error,
  loading,
  run,
  first,
  search,
  onPage,
} = useIndexListing({
  routeName: 'objects',
  pageSize: PAGE_SIZE,
  query: () => ({
    q: query.value || undefined,
    catalog: catalog.value || undefined,
    sort: sort.value === 'entries' ? undefined : sort.value,
  }),
  fetch: (offset, signal) =>
    api.objects(
      {
        q: query.value || undefined,
        catalog: catalog.value || undefined,
        sort: sort.value,
        offset,
        limit: PAGE_SIZE,
      },
      signal,
    ),
})

onMounted(() => {
  void run()
  api
    .objectCatalogs()
    .then((found) => {
      catalogs.value = found
    })
    .catch(() => {})
})

const catalogOptions = computed(() =>
  catalogs.value.map((entry) => ({
    label: `${nameOf(entry.catalog)} (${entry.objects.toLocaleString()})`,
    value: entry.catalog,
  })),
)

function nameOf(value: string): string {
  return CATALOGS[value] ?? value
}

function span(first: string, last: string): string {
  const from = yearOf(first)
  const to = yearOf(last)
  return from === to ? String(from) : `${from} to ${to}`
}
</script>

<template>
  <div class="stack">
    <h1>Objects</h1>

    <IndexTabs />

    <div class="row controls">
      <IconField class="search">
        <InputIcon class="pi pi-search" />
        <InputText
          v-model="query"
          aria-label="Search objects"
          fluid
          placeholder="Search a designation or a name…"
          type="search"
          @input="search(true)"
        />
      </IconField>

      <Select
        v-model="catalog"
        :options="catalogOptions"
        aria-label="Catalogue"
        class="catalog"
        option-label="label"
        option-value="value"
        placeholder="Any catalogue"
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
      {{ listing.total === 1 ? 'object' : 'objects' }}
    </p>

    <div v-if="loading && !listing" class="stack lines">
      <Skeleton v-for="index in 8" :key="index" height="3.4rem" width="100%" />
    </div>

    <p v-else-if="listing && !listing.items.length" class="muted empty">No object matches that.</p>

    <ul v-else-if="listing" class="stack results">
      <IndexRow
        v-for="object in listing.items"
        :key="object.id"
        :name="object.id"
        :to="`/objects/${encodeURIComponent(object.id)}`"
      >
        <template #note>{{ nameOf(object.catalog) }}</template>
        <template #meta>
          <span class="muted years">{{ span(object.first, object.last) }}</span>
          <span class="entries">
            {{ object.entries.toLocaleString() }}
            <span class="muted unit">{{ object.entries === 1 ? 'entry' : 'entries' }}</span>
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

.controls :deep(.p-inputtext),
.controls :deep(.p-select) {
  height: 2.75rem;
}

.controls :deep(.p-select-label) {
  display: flex;
  align-items: center;
}

.catalog {
  min-width: 12rem;
}

.sort {
  min-width: 12rem;
}

.count {
  margin: 0;
  font-size: var(--text-sm);
}

.results {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: var(--space-2);
}

.years {
  font-size: var(--text-sm);
}

.entries {
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
  .catalog,
  .sort {
    width: 100%;
  }
}
</style>
