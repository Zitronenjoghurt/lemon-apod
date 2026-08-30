<script lang="ts" setup>
import { computed, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import FieldChange from '@/components/FieldChange.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import { useAsync } from '@/composables/useAsync'
import { useNarrow } from '@/composables/useNarrow'
import { formatDate } from '@/utils/date'

defineOptions({ name: 'ChangesView' })

const PAGE_SIZE = 25

const route = useRoute()
const router = useRouter()
const { pageLinks } = useNarrow()

const field = computed(() => (route.query.field as string | undefined) || undefined)
const offset = ref(0)

const { data, error, loading, run } = useAsync((signal) =>
  api.divergences({ field: field.value, offset: offset.value, limit: PAGE_SIZE }, signal),
)

const { data: summary, run: loadSummary } = useAsync((signal) => api.migration(signal))

const fields = computed(() => summary.value?.divergences ?? [])

function label(name: string): string {
  return name.replace(/_/g, ' ')
}

function choose(next?: string) {
  offset.value = 0
  void router.replace({ path: '/modernization/changes', query: next ? { field: next } : {} })
}

function page(event: { first: number }) {
  offset.value = event.first
  void run()
}

watch(field, () => {
  offset.value = 0
  void run()
})

onMounted(() => {
  void run()
  void loadSummary()
})
</script>

<template>
  <div class="stack changes">
    <header class="stack head">
      <RouterLink class="back muted" to="/modernization">
        <i aria-hidden="true" class="pi pi-angle-left" />
        Modernization
      </RouterLink>
      <h1>What changed</h1>
    </header>

    <nav v-if="fields.length" aria-label="Filter by field" class="row filters">
      <button :class="{ on: !field }" type="button" @click="choose(undefined)">
        Everything
        <span class="muted tally">{{ summary?.differences?.toLocaleString() }}</span>
      </button>
      <button
        v-for="row in fields"
        :key="row.field"
        :class="{ on: field === row.field }"
        type="button"
        @click="choose(row.field)"
      >
        {{ label(row.field) }}
        <span class="muted tally">{{ row.entries.toLocaleString() }}</span>
      </button>
    </nav>

    <RetryNotice v-if="error" :busy="loading" :message="error" @retry="run" />

    <p v-else-if="data && !data.items.length" class="muted">Nothing recorded yet.</p>

    <ol v-else-if="data" class="stack rows">
      <li v-for="row in data.items" :key="`${row.date}:${row.field}`" class="card row-card">
        <div class="row when">
          <RouterLink :to="`/${row.date}`">{{ formatDate(row.date) }}</RouterLink>
          <a :href="`/${row.date}/original`" class="muted original">
            Original page
            <i aria-hidden="true" class="pi pi-external-link" />
          </a>
        </div>
        <p class="title">{{ row.title }}</p>

        <FieldChange :row="row" />
      </li>
    </ol>

    <Paginator
      v-if="data && data.total > PAGE_SIZE"
      :first="offset"
      :pageLinkSize="pageLinks"
      :rows="PAGE_SIZE"
      :totalRecords="data.total"
      @page="page"
    />
  </div>
</template>

<style scoped>
.changes {
  max-width: 62rem;
  margin-inline: auto;
  gap: 1.2rem;
}

h1 {
  font-size: 1.6rem;
}

.back {
  display: inline-flex;
  align-items: center;
  gap: 0.15rem;
  width: fit-content;
  font-size: 0.85rem;
  text-decoration: none;
  color: var(--text-muted);
}

.back:hover {
  color: var(--text);
}

.lead {
  margin: 0;
  max-width: 60ch;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.filters {
  gap: 0.4rem;
  flex-wrap: wrap;
}

.filters button {
  display: inline-flex;
  align-items: baseline;
  gap: 0.4rem;
  padding: 0.3rem 0.7rem;
  border: 1px solid var(--border);
  border-radius: 1.1rem;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 0.85rem;
  cursor: pointer;
}

.filters button.on {
  border-color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}

.tally {
  font-variant-numeric: tabular-nums;
  font-size: 0.8em;
}

.rows {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: 0.7rem;
}

.row-card {
  padding: 0.8rem 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.when {
  gap: 0.6rem;
  flex-wrap: wrap;
  align-items: baseline;
  font-size: 0.85rem;
}

.original {
  margin-left: auto;
  white-space: nowrap;
}

.original i {
  font-size: 0.7em;
}

.title {
  margin: 0;
  font-weight: 600;
  text-wrap: balance;
}

@media (max-width: 44rem) {
  .original {
    margin-left: 0;
  }
}
</style>
