<script lang="ts" setup>
import { computed, watch } from 'vue'
import { RouterLink } from 'vue-router'
import RetryNotice from './RetryNotice.vue'
import YearChart from './YearChart.vue'
import { api } from '@/api/client'
import { useAsync } from '@/composables/useAsync'
import { formatDate } from '@/utils/date'

const props = defineProps<{ word?: string }>()
const emit = defineEmits<{ close: [] }>()

const {
  data: use,
  error,
  notFound,
  loading,
  run,
} = useAsync((signal) => api.word(props.word ?? '', signal))

watch(
  () => props.word,
  (word) => {
    if (word) void run()
  },
  { immediate: true },
)

const open = computed({
  get: () => props.word !== undefined,
  set: (value: boolean) => {
    if (!value) emit('close')
  },
})

const points = computed(() =>
  (use.value?.by_year ?? []).map((year) => ({ year: year.year, value: year.total })),
)

/// A word used in half the archive says nothing about any one entry; a word used in three says
/// a lot. This is the number worth leading with.
const reach = computed(() => use.value?.entries ?? 0)
</script>

<template>
  <Dialog
    v-model:visible="open"
    :header="word ?? ''"
    :style="{ width: 'min(38rem, 92vw)' }"
    dismissable-mask
    modal
  >
    <RetryNotice v-if="error" :busy="loading" :message="error" @retry="run" />

    <p v-else-if="notFound" class="muted">Nothing in the archive uses that word.</p>

    <div v-else-if="loading && !use" class="stack">
      <Skeleton height="2.5rem" width="100%" />
      <Skeleton height="7rem" width="100%" />
    </div>

    <div v-else-if="use" class="stack body">
      <dl class="facts">
        <div>
          <dt>Used</dt>
          <dd>{{ use.total.toLocaleString() }}&times;</dd>
        </div>
        <div>
          <dt>Entries</dt>
          <dd>{{ reach.toLocaleString() }}</dd>
        </div>
        <div v-if="use.first">
          <dt>First</dt>
          <dd class="date">{{ formatDate(use.first) }}</dd>
        </div>
        <div v-if="use.last">
          <dt>Last</dt>
          <dd class="date">{{ formatDate(use.last) }}</dd>
        </div>
      </dl>

      <YearChart v-if="points.length > 1" :points="points" label="Times used per year" />

      <div v-if="use.top_entries.length" class="stack top">
        <h3>Leaning on it hardest</h3>
        <ul>
          <li v-for="entry in use.top_entries" :key="entry.date">
            <RouterLink :to="`/${entry.date}?q=${encodeURIComponent(use.word)}`">
              {{ entry.title }}
            </RouterLink>
            <span class="muted count">{{ entry.count }}&times;</span>
          </li>
        </ul>
      </div>
    </div>
  </Dialog>
</template>

<style scoped>
.body {
  gap: 1.2rem;
}

.facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(6rem, 1fr));
  gap: 0.6rem 1rem;
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

.top {
  gap: 0.4rem;
}

h3 {
  font-size: 0.78rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  font-weight: 600;
}

ul {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  font-size: 0.9rem;
}

li {
  display: flex;
  justify-content: space-between;
  gap: 0.75rem;
}

.count {
  font-variant-numeric: tabular-nums;
  flex: none;
}
</style>
