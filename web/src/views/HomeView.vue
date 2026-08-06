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
    <Button label="Try again" icon="pi pi-refresh" outlined @click="run" />
  </div>

  <EntryDetail v-else-if="entry" :entry="entry" :latest="entry.date" />
</template>

<style scoped>
.notice {
  padding: 2rem;
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
}

.notice p {
  margin: 0;
}
</style>
