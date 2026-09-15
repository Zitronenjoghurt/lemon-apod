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

const CATALOGS: Record<string, string> = {
  messier: 'Messier catalogue',
  ngc: 'New General Catalogue',
  ic: 'Index Catalogue',
  sharpless: 'Sharpless catalogue',
  abell: 'Abell catalogue',
  caldwell: 'Caldwell catalogue',
  barnard: 'Barnard catalogue',
  arp: 'Arp catalogue',
  ldn: 'Lynds catalogue of dark nebulae',
  lbn: 'Lynds catalogue of bright nebulae',
  vdb: 'van den Bergh catalogue',
  cederblad: 'Cederblad catalogue',
  melotte: 'Melotte catalogue',
  collinder: 'Collinder catalogue',
  trumpler: 'Trumpler catalogue',
  hickson: 'Hickson compact groups',
  terzan: 'Terzan catalogue',
  palomar: 'Palomar globulars',
  ugc: 'Uppsala General Catalogue',
  pgc: 'Principal Galaxies Catalogue',
  rcw: 'RCW catalogue',
  gum: 'Gum catalogue',
  simeis: 'Simeis catalogue',
  'herbig-haro': 'Herbig-Haro objects',
  supernova: 'Supernova',
  comet: 'Comet',
  solar: 'The solar system',
  star: 'Star',
  shower: 'Meteor shower',
  constellation: 'Constellation',
  named: 'Named without a catalogue',
}

const route = useRoute()
const { pageLinks } = useNarrow()
const id = computed(() => String(route.params.id))
const page = ref(1)

const { data, error, notFound, loading, run } = useAsync((signal) =>
  api.object(id.value, { offset: (page.value - 1) * PAGE_SIZE, limit: PAGE_SIZE }, signal),
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

const object = computed(() => data.value?.object)

const catalog = computed(() => {
  const value = object.value?.catalog
  return (value && CATALOGS[value]) || value || ''
})

const total = computed(() => object.value?.entries ?? 0)

watch([object, notFound], ([found, missing]) => {
  if (found) setTitle(pageTitle(found.id))
  else if (missing) setTitle(pageTitle('Object not found'))
})
</script>

<template>
  <div class="stack">
    <RouterLink class="muted back" to="/objects">
      <AppIcon name="arrow-left" /> All objects
    </RouterLink>

    <div v-if="notFound" class="card notice">
      <h1>No such object</h1>
      <p class="muted">
        Objects are read out of the entries whenever the parser changes, so an old link into them
        can go stale.
      </p>
      <RouterLink class="plain" to="/objects">
        <Button label="Back to the objects" outlined tabindex="-1">
          <template #icon><AppIcon name="arrow-left" /></template>
        </Button>
      </RouterLink>
    </div>

    <RetryNotice v-else-if="error" :busy="loading" :message="error" @retry="run" />

    <div v-else-if="!data" aria-busy="true" aria-label="Loading the object" class="stack">
      <Skeleton height="11rem" width="100%" />
      <Skeleton height="18rem" width="100%" />
    </div>

    <template v-else-if="object">
      <header class="card stack head">
        <h1>{{ object.id }}</h1>
        <p class="muted catalog">{{ catalog }}</p>

        <dl class="facts">
          <div>
            <dt>Entries</dt>
            <dd>{{ object.entries.toLocaleString() }}</dd>
          </div>
          <div>
            <dt>First</dt>
            <dd class="date">{{ formatDate(object.first) }}</dd>
          </div>
          <div>
            <dt>Last</dt>
            <dd class="date">{{ formatDate(object.last) }}</dd>
          </div>
        </dl>
      </header>

      <EntryGrid :entries="data.items" :loading="loading" empty="No entry names this one." />

      <Paginator
        v-if="total > PAGE_SIZE"
        :first="(page - 1) * PAGE_SIZE"
        :page-link-size="pageLinks"
        :rows="PAGE_SIZE"
        :total-records="total"
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
  font-variant-numeric: tabular-nums;
}

.catalog {
  margin: 0;
  font-size: var(--text-sm);
}

.facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 8rem), 1fr));
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
