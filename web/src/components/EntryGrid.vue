<script lang="ts" setup>
import ApodCredit from './ApodCredit.vue'
import EntryCard from './EntryCard.vue'
import type { ApodSummary, SearchHit } from '@/api/types'

withDefaults(
  defineProps<{
    entries?: (ApodSummary | SearchHit)[]
    loading?: boolean
    placeholders?: number
    empty?: string
    query?: string
    credit?: boolean
  }>(),
  {
    entries: () => [],
    loading: false,
    placeholders: 8,
    empty: 'Nothing here.',
    query: undefined,
    credit: true,
  },
)

const STAGGER_CAP = 11

function snippetOf(entry: ApodSummary | SearchHit): string | undefined {
  return 'snippet' in entry ? entry.snippet : undefined
}

function hitOf(entry: ApodSummary | SearchHit): SearchHit | undefined {
  return 'matched' in entry ? entry : undefined
}
</script>

<template>
  <div v-if="loading && !entries.length" aria-busy="true" aria-label="Loading entries" class="grid">
    <div v-for="index in placeholders" :key="index" class="card skeleton-card">
      <Skeleton class="thumb" height="0" width="100%" />
      <div class="lines">
        <Skeleton height="0.8rem" width="40%" />
        <Skeleton height="0.8rem" width="100%" />
      </div>
    </div>
  </div>

  <p v-else-if="!entries.length" class="muted empty">{{ empty }}</p>

  <template v-else>
    <ApodCredit v-if="credit" variant="banner" />

    <div class="grid">
      <EntryCard
        v-for="(entry, index) in entries"
        :key="entry.date"
        :entry="entry"
        :hit="hitOf(entry)"
        :query="query"
        :snippet="snippetOf(entry)"
        :style="{ '--rise-delay': `${Math.min(index, STAGGER_CAP) * 30}ms` }"
      />
    </div>
  </template>
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
  padding: var(--space-4) var(--space-4) var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.empty {
  padding: var(--space-8) 0;
  text-align: center;
}
</style>
