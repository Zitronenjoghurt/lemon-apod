<script lang="ts" setup>
import { computed, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import EntryDetail from '@/components/EntryDetail.vue'
import EntrySkeleton from '@/components/EntrySkeleton.vue'
import { api } from '@/api/client'
import { useAsync } from '@/composables/useAsync'
import { useLatestDate } from '@/composables/useLatestDate'
import { formatDate } from '@/utils/date'

const route = useRoute()
const latest = useLatestDate()
const date = computed(() => String(route.params.date ?? ''))

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
</script>

<template>
  <EntrySkeleton v-if="loading && !entry" />

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
  max-width: 40ch;
  margin: 0;
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}
</style>
