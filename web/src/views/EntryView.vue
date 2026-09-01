<script lang="ts" setup>
import { computed, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import EntryDetail from '@/components/EntryDetail.vue'
import EntrySkeleton from '@/components/EntrySkeleton.vue'
import GapDetail from '@/components/GapDetail.vue'
import { api } from '@/api/client'
import { useAsync } from '@/composables/useAsync'
import { useGaps } from '@/composables/useGaps'
import { useLatestDate } from '@/composables/useStatus'
import { formatDate } from '@/utils/date'
import { entryTitle, gapTitle, pageTitle, setTitle } from '@/utils/title'

const route = useRoute()
const latest = useLatestDate()
const { gaps, loaded: gapsLoaded } = useGaps()
const date = computed(() => String(route.params.date ?? ''))

const gap = computed(() => gaps.value.find((one) => one.date === date.value) ?? null)

const highlight = computed(() => {
  const raw = String(route.query.q ?? '').trim()
  return raw || undefined
})

const {
  data: entry,
  error,
  notFound,
  loading,
  run,
} = useAsync((signal) => api.entry(date.value, signal))

watch(date, run, { immediate: true })

watch([entry, notFound, gap], ([found, missing, empty]) => {
  if (found) setTitle(entryTitle(found.title, found.date))
  else if (empty) setTitle(gapTitle(empty.date))
  else if (missing) setTitle(pageTitle(`No entry for ${formatDate(date.value)}`))
})
</script>

<template>
  <GapDetail v-if="gap" :gap="gap" />

  <EntrySkeleton v-else-if="(loading && !entry) || (notFound && !gapsLoaded)" />

  <div v-else-if="notFound" class="card notice">
    <h1>No entry for {{ formatDate(date) }}</h1>
    <p class="muted">
      Either APOD published nothing that day, or the archiver has not reached it yet. It walks
      backwards from today, so older dates arrive last.
    </p>
    <RouterLink class="plain" to="/">
      <Button icon="pi pi-arrow-left" label="Back to the latest entry" outlined tabindex="-1" />
    </RouterLink>
  </div>

  <div v-else-if="error" class="card notice">
    <p>{{ error }}</p>
    <Button icon="pi pi-refresh" label="Try again" outlined @click="run" />
  </div>

  <EntryDetail
    v-else-if="entry"
    :entry="entry"
    :highlight="highlight"
    :latest="latest ?? undefined"
  />
</template>

<style scoped>
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
  max-width: 40ch;
  margin: 0;
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}
</style>
