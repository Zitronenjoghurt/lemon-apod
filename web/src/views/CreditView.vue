<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import EntryGrid from '@/components/EntryGrid.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import { useAsync } from '@/composables/useAsync'
import { useNarrow } from '@/composables/useNarrow'
import { formatDate } from '@/utils/date'
import { pageTitle, setTitle } from '@/utils/title'

const PAGE_SIZE = 24

const route = useRoute()
const { pageLinks } = useNarrow()
const id = computed(() => String(route.params.id))
const page = ref(1)

const { data, error, notFound, loading, run } = useAsync((signal) =>
  api.credit(id.value, (page.value - 1) * PAGE_SIZE, PAGE_SIZE, signal),
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

const contributor = computed(() => data.value?.contributor)
const name = computed(() => contributor.value?.label ?? '')

const kind = computed(() => {
  const value = contributor.value?.kind
  if (value === 'person') return 'Person'
  if (value === 'group') return 'Institution, mission or instrument'
  return ''
})

const ROLES_SHOWN = 6

const roles = computed(() => {
  const all = data.value?.roles ?? []
  const shown = all
    .slice(0, ROLES_SHOWN)
    .map((row) => (row.entries > 1 ? `${row.role} ×${row.entries}` : row.role))

  const rest = all.length - shown.length
  if (rest > 0) shown.push(`and ${rest} more`)

  return shown.join(' · ')
})

watch([name, notFound], ([named, missing]) => {
  if (named) setTitle(pageTitle(named))
  else if (missing) setTitle(pageTitle('Credit not found'))
})
</script>

<template>
  <div class="stack">
    <RouterLink class="muted back" to="/credits">
      <i aria-hidden="true" class="pi pi-arrow-left" /> All credits
    </RouterLink>

    <div v-if="notFound" class="card notice">
      <h1>No such credit</h1>
      <p class="muted">
        Credits are gathered from the entries whenever the parser changes, so an old link into them
        can go stale.
      </p>
      <RouterLink class="plain" to="/credits">
        <Button icon="pi pi-arrow-left" label="Back to the credits" outlined tabindex="-1" />
      </RouterLink>
    </div>

    <RetryNotice v-else-if="error" :busy="loading" :message="error" @retry="run" />

    <div v-else-if="!data" aria-busy="true" aria-label="Loading the credit" class="stack">
      <Skeleton height="11rem" width="100%" />
      <Skeleton height="18rem" width="100%" />
    </div>

    <template v-else-if="contributor">
      <header class="card stack head">
        <h1>{{ name }}</h1>
        <p v-if="kind" class="muted kind">{{ kind }}</p>

        <dl class="facts">
          <div>
            <dt>Entries</dt>
            <dd>{{ contributor.entries.toLocaleString() }}</dd>
          </div>
          <div>
            <dt>First</dt>
            <dd class="date">{{ formatDate(contributor.first) }}</dd>
          </div>
          <div>
            <dt>Last</dt>
            <dd class="date">{{ formatDate(contributor.last) }}</dd>
          </div>
        </dl>

        <div v-if="roles" class="stack roles">
          <h2>Credited as</h2>
          <p>{{ roles }}</p>
        </div>

        <a
          v-if="contributor.url"
          :href="contributor.url"
          class="site"
          rel="noopener nofollow"
          target="_blank"
        >
          {{ contributor.url }} <i aria-hidden="true" class="pi pi-external-link" />
        </a>
      </header>

      <h2 class="section">
        {{ contributor.entries.toLocaleString() }}
        {{ contributor.entries === 1 ? 'entry' : 'entries' }}
      </h2>

      <EntryGrid :entries="data.items" :loading="loading" empty="Nothing credits this." />

      <Paginator
        v-if="contributor.entries > PAGE_SIZE"
        :first="(page - 1) * PAGE_SIZE"
        :page-link-size="pageLinks"
        :rows="PAGE_SIZE"
        :total-records="contributor.entries"
        @page="onPage"
      />
    </template>
  </div>
</template>

<style scoped>
.back {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-sm);
  text-decoration: none;
  align-self: flex-start;
}

.back:hover {
  color: var(--text);
}

.head {
  padding: var(--space-5);
  gap: var(--space-3);
}

h1 {
  font-size: var(--text-title);
  text-wrap: balance;
}

.kind {
  margin: 0;
  font-size: var(--text-sm);
}

.facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 7rem), 1fr));
  gap: var(--space-3) var(--space-5);
  margin: 0;
}

.facts dt {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
}

.facts dd {
  margin: 0;
  font-size: var(--text-lg);
  font-variant-numeric: tabular-nums;
}

.facts .date {
  font-size: var(--text-md);
}

.roles {
  gap: var(--space-1);
}

.roles h2,
h2.section {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  font-weight: 600;
}

h2.section {
  margin-bottom: -0.5rem;
}

.roles p {
  margin: 0;
  font-size: var(--text-sm);
  font-variant-numeric: tabular-nums;
}

.site {
  font-size: var(--text-sm);
  align-self: flex-start;
  text-decoration: none;
  word-break: break-all;
}

.site:hover {
  text-decoration: underline;
}

.site i {
  font-size: 0.7em;
}

.notice {
  padding: var(--space-8) var(--space-7);
  text-align: center;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  align-items: center;
}

.notice h1 {
  font-size: var(--text-title);
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
