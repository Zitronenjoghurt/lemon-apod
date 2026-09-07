<script lang="ts" setup>
import { onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import IndexRow from '@/components/IndexRow.vue'
import IndexTabs from '@/components/IndexTabs.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import type { ContributorKind, CreditSort } from '@/api/types'
import { useIndexListing } from '@/composables/useIndexListing'
import { useNarrow } from '@/composables/useNarrow'
import { year as yearOf } from '@/utils/date'

const PAGE_SIZE = 30

const KINDS: { label: string; value: ContributorKind }[] = [
  { label: 'People', value: 'person' },
  { label: 'Institutions and missions', value: 'group' },
]

const SORTS: { label: string; value: CreditSort }[] = [
  { label: 'Most credited', value: 'entries' },
  { label: 'Credited most recently', value: 'latest' },
  { label: 'Name, A to Z', value: 'name' },
]

const route = useRoute()
const { pageLinks } = useNarrow()

const query = ref(String(route.query.q ?? ''))
const kind = ref<ContributorKind | null>(
  KINDS.some((option) => option.value === route.query.kind)
    ? (route.query.kind as ContributorKind)
    : null,
)
const sort = ref<CreditSort>(
  SORTS.some((option) => option.value === route.query.sort)
    ? (route.query.sort as CreditSort)
    : 'entries',
)

const {
  data: listing,
  error,
  loading,
  run,
  first,
  search,
  onPage,
} = useIndexListing({
  routeName: 'credits',
  pageSize: PAGE_SIZE,
  query: () => ({
    q: query.value || undefined,
    kind: kind.value || undefined,
    sort: sort.value === 'entries' ? undefined : sort.value,
  }),
  fetch: (offset, signal) =>
    api.credits(
      {
        q: query.value || undefined,
        kind: kind.value || undefined,
        sort: sort.value,
        offset,
        limit: PAGE_SIZE,
      },
      signal,
    ),
})

onMounted(() => {
  void run()
})

function span(first: string, last: string): string {
  const from = yearOf(first)
  const to = yearOf(last)
  return from === to ? String(from) : `${from} to ${to}`
}

function kindOf(value: string): string {
  if (value === 'person') return 'Person'
  if (value === 'group') return 'Institution, mission or instrument'
  return ''
}
</script>

<template>
  <div class="stack">
    <h1>Credits</h1>

    <IndexTabs />

    <div class="row controls">
      <IconField class="search">
        <InputIcon class="pi pi-search" />
        <InputText
          v-model="query"
          aria-label="Search credits"
          fluid
          placeholder="Search a name…"
          type="search"
          @input="search(true)"
        />
      </IconField>

      <Select
        v-model="kind"
        :options="KINDS"
        aria-label="Kind"
        class="kind"
        option-label="label"
        option-value="value"
        placeholder="Anyone"
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
      {{ listing.total === 1 ? 'credit' : 'credits' }}
    </p>

    <div v-if="loading && !listing" class="stack lines">
      <Skeleton v-for="index in 8" :key="index" height="3.4rem" width="100%" />
    </div>

    <p v-else-if="listing && !listing.items.length" class="muted empty">No credit matches that.</p>

    <ul v-else-if="listing" class="stack results">
      <IndexRow
        v-for="contributor in listing.items"
        :key="contributor.id"
        :name="contributor.label"
        :to="`/credits/${encodeURIComponent(contributor.id)}`"
      >
        <template v-if="kindOf(contributor.kind)" #note>
          {{ kindOf(contributor.kind) }}
        </template>
        <template #meta>
          <span class="muted years">{{ span(contributor.first, contributor.last) }}</span>
          <span class="entries">
            {{ contributor.entries.toLocaleString() }}
            <span class="muted unit">{{ contributor.entries === 1 ? 'entry' : 'entries' }}</span>
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

.kind {
  min-width: 13rem;
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
  .unit {
    display: none;
  }

  .kind,
  .sort {
    width: 100%;
  }
}
</style>
