<script setup lang="ts">
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
    <RouterLink to="/">Back to the latest entry</RouterLink>
  </div>

  <div v-else-if="error" class="card notice">
    <p>{{ error }}</p>
    <button type="button" @click="run">Try again</button>
  </div>

  <EntryDetail v-else-if="entry" :entry="entry" :latest="latest ?? undefined" />
</template>

<style scoped>
.notice {
  padding: 3rem 2rem;
  text-align: center;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  align-items: center;
}

.notice h1 {
  font-size: 1.4rem;
}

.notice p {
  max-width: 40ch;
  margin: 0;
}

.notice button {
  font: inherit;
  padding: 0.4rem 1rem;
  border-radius: 0.6rem;
  border: 1px solid var(--border);
  background: var(--bg);
  color: inherit;
  cursor: pointer;
}
</style>
