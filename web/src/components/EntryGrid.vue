<script setup lang="ts">
import EntryCard from './EntryCard.vue'
import type { ApodSummary, SearchHit } from '@/api/types'

withDefaults(
  defineProps<{
    entries?: (ApodSummary | SearchHit)[]
    loading?: boolean
    placeholders?: number
    empty?: string
  }>(),
  { entries: () => [], loading: false, placeholders: 8, empty: 'Nothing here.' },
)

function snippetOf(entry: ApodSummary | SearchHit): string | undefined {
  return 'snippet' in entry ? entry.snippet : undefined
}
</script>

<template>
  <div v-if="loading && !entries.length" class="grid" aria-busy="true" aria-label="Loading entries">
    <div v-for="index in placeholders" :key="index" class="card skeleton-card">
      <Skeleton width="100%" height="0" class="thumb" />
      <div class="lines">
        <Skeleton width="40%" height="0.8rem" />
        <Skeleton width="100%" height="0.8rem" />
      </div>
    </div>
  </div>

  <p v-else-if="!entries.length" class="muted empty">{{ empty }}</p>

  <div v-else class="grid">
    <EntryCard
      v-for="entry in entries"
      :key="entry.date"
      :entry="entry"
      :snippet="snippetOf(entry)"
    />
  </div>
</template>

<style scoped>
.skeleton-card {
  overflow: hidden;
}

.thumb {
  aspect-ratio: 16 / 10;
  border-radius: 0;
}

.lines {
  padding: 0.9rem 1rem 1.2rem;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.empty {
  padding: 3rem 0;
  text-align: center;
}
</style>
