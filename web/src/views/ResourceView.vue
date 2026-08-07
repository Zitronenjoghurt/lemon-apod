<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import EntryGrid from '@/components/EntryGrid.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import { useAsync } from '@/composables/useAsync'
import { formatDate } from '@/utils/date'

const PAGE_SIZE = 30

const route = useRoute()
const id = computed(() => Number(route.params.id))
const page = ref(1)

const { data, error, notFound, loading, run } = useAsync((signal) =>
  api.resource(id.value, (page.value - 1) * PAGE_SIZE, PAGE_SIZE, signal),
)

watch(
  id,
  () => {
    page.value = 1
    void run()
  },
  { immediate: true },
)

function onPage(event: { page: number }) {
  page.value = event.page + 1
  void run()
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

const resource = computed(() => data.value?.resource)

const address = computed(() => resource.value?.key ?? '')

const name = computed(() => resource.value?.label?.trim() || address.value)

const anchors = computed(() => {
  const counts = new Map<string, number>()
  for (const item of data.value?.items ?? []) {
    const anchor = item.anchor.trim()
    if (anchor) counts.set(anchor, (counts.get(anchor) ?? 0) + 1)
  }
  return [...counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8)
})
</script>

<template>
  <div class="stack">
    <RouterLink class="muted back" to="/resources">
      <i aria-hidden="true" class="pi pi-arrow-left" /> All resources
    </RouterLink>

    <div v-if="notFound" class="card notice">
      <h1>No such resource</h1>
      <p class="muted">
        The catalogue is rebuilt whenever the parser changes, so an old link into it can go stale.
      </p>
      <RouterLink class="plain" to="/resources">
        <Button icon="pi pi-arrow-left" label="Back to the catalogue" outlined tabindex="-1" />
      </RouterLink>
    </div>

    <RetryNotice v-else-if="error" :busy="loading" :message="error" @retry="run" />

    <div v-else-if="!data" aria-busy="true" aria-label="Loading the resource" class="stack">
      <Skeleton height="13rem" width="100%" />
      <Skeleton height="18rem" width="100%" />
    </div>

    <template v-else-if="resource">
      <header class="card stack head">
        <h1>{{ name }}</h1>
        <a :href="resource.url" class="address" rel="noopener nofollow" target="_blank">
          {{ address }} <i aria-hidden="true" class="pi pi-external-link" />
        </a>

        <dl class="facts">
          <div>
            <dt>References</dt>
            <dd>{{ resource.refs.toLocaleString() }}</dd>
          </div>
          <div>
            <dt>Entries</dt>
            <dd>{{ resource.entries.toLocaleString() }}</dd>
          </div>
          <div v-if="resource.credited">
            <dt>From credits</dt>
            <dd>{{ resource.credited.toLocaleString() }}</dd>
          </div>
          <div v-if="resource.first">
            <dt>First</dt>
            <dd class="date">{{ formatDate(resource.first) }}</dd>
          </div>
          <div v-if="resource.last">
            <dt>Last</dt>
            <dd class="date">{{ formatDate(resource.last) }}</dd>
          </div>
        </dl>

        <div v-if="anchors.length > 1" class="stack anchors">
          <h2>Called</h2>
          <ul class="row">
            <li v-for="[anchor, times] in anchors" :key="anchor">
              {{ anchor }}<span v-if="times > 1" class="muted times"> &times;{{ times }}</span>
            </li>
          </ul>
        </div>

        <RouterLink :to="{ path: '/resources', query: { host: resource.host } }" class="muted site">
          More from {{ resource.host }}
        </RouterLink>
      </header>

      <h2 class="section">
        Referenced by {{ data.total.toLocaleString() }}
        {{ data.total === 1 ? 'entry' : 'entries' }}
      </h2>

      <EntryGrid :entries="data.items" :loading="loading" empty="Nothing references this." />

      <Paginator
        v-if="data.total > PAGE_SIZE"
        :first="(page - 1) * PAGE_SIZE"
        :rows="PAGE_SIZE"
        :total-records="data.total"
        @page="onPage"
      />
    </template>
  </div>
</template>

<style scoped>
.back {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.85rem;
  text-decoration: none;
  align-self: flex-start;
}

.back:hover {
  color: var(--text);
}

.head {
  padding: 1.2rem 1.3rem 1.3rem;
  gap: 0.7rem;
}

h1 {
  font-size: 1.45rem;
  text-wrap: balance;
}

.address {
  font-size: 0.88rem;
  word-break: break-all;
  text-decoration: none;
}

.address:hover {
  text-decoration: underline;
}

.address i {
  font-size: 0.7em;
}

.facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 7rem), 1fr));
  gap: 0.7rem 1.2rem;
  margin: 0;
}

.facts dt {
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
}

.facts dd {
  margin: 0;
  font-size: 1.2rem;
  font-variant-numeric: tabular-nums;
}

.facts .date {
  font-size: 0.95rem;
}

.anchors {
  gap: 0.35rem;
}

.anchors h2,
h2.section {
  font-size: 0.78rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  font-weight: 600;
}

h2.section {
  margin-bottom: -0.5rem;
}

.anchors ul {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: 0.35rem;
  font-size: 0.85rem;
}

.anchors li {
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 0.1rem 0.6rem;
}

.times {
  font-variant-numeric: tabular-nums;
}

.site {
  font-size: 0.85rem;
  align-self: flex-start;
  text-decoration: none;
}

.site:hover {
  color: var(--accent);
}

.notice {
  padding: 3rem 2rem;
  text-align: center;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: center;
}

.notice h1 {
  font-size: 1.4rem;
}

.notice p {
  max-width: 44ch;
  margin: 0;
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}
</style>
