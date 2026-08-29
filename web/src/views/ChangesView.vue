<script lang="ts" setup>
import { computed, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
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
          <span class="field">{{ label(row.field) }}</span>
          <a :href="`/${row.date}/original`" class="muted original">
            Original page
            <i aria-hidden="true" class="pi pi-external-link" />
          </a>
        </div>
        <p class="title">{{ row.title }}</p>

        <div class="change">
          <div class="side before">
            <span class="side-label">
              <i aria-hidden="true" class="pi pi-check-circle" />
              apod.nasa.gov
            </span>
            <p class="side-value">{{ row.legacy || '—' }}</p>
          </div>

          <i aria-hidden="true" class="pi pi-arrow-right arrow" />

          <div class="side after">
            <span class="side-label">
              <i aria-hidden="true" class="pi pi-globe" />
              science.nasa.gov
            </span>
            <p class="side-value">{{ row.modern || '—' }}</p>
          </div>
        </div>
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
  gap: 0.5rem;
}

.when {
  gap: 0.6rem;
  flex-wrap: wrap;
  align-items: baseline;
  font-size: 0.85rem;
}

.field {
  padding: 0.1rem 0.5rem;
  border-radius: 0.8rem;
  background: color-mix(in srgb, var(--text) 8%, transparent);
  font-size: 0.75rem;
  letter-spacing: 0.04em;
  text-transform: uppercase;
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

.change {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  gap: 0.75rem;
}

.side {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  min-width: 0;
  padding: 0.5rem 0.7rem;
  border-radius: var(--radius);
  border: 1px solid var(--border);
}

.side.before {
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
  background: color-mix(in srgb, var(--accent) 7%, transparent);
}

.side-label {
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.7rem;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.side.before .side-label i {
  color: var(--accent);
}

.side-label em {
  font-style: normal;
  text-transform: none;
  letter-spacing: 0;
  opacity: 0.75;
}

.side-value {
  margin: 0;
  font-size: 0.9rem;
  overflow-wrap: anywhere;
}

.side.after .side-value {
  color: var(--text-muted);
}

.arrow {
  color: var(--text-muted);
  font-size: 0.9rem;
}

@media (max-width: 44rem) {
  .change {
    grid-template-columns: minmax(0, 1fr);
    gap: 0.35rem;
    justify-items: stretch;
  }

  .arrow {
    justify-self: center;
    transform: rotate(90deg);
  }

  .original {
    margin-left: 0;
  }
}
</style>
