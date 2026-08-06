<script setup lang="ts">
import { onMounted } from 'vue'
import EntryDetail from '@/components/EntryDetail.vue'
import EntrySkeleton from '@/components/EntrySkeleton.vue'
import { api } from '@/api/client'
import { useAsync } from '@/composables/useAsync'

const { data: entry, error, loading, run } = useAsync((signal) => api.latest(signal))

onMounted(run)
</script>

<template>
  <EntrySkeleton v-if="loading && !entry" />

  <div v-else-if="error" class="card notice">
    <p>{{ error }}</p>
    <button type="button" @click="run">Try again</button>
  </div>

  <EntryDetail v-else-if="entry" :entry="entry" :latest="entry.date" />
</template>

<style scoped>
.notice {
  padding: 2rem;
  text-align: center;
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
