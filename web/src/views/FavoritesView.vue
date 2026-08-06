<script setup lang="ts">
import { ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import EntryGrid from '@/components/EntryGrid.vue'
import { api } from '@/api/client'
import type { ApodSummary } from '@/api/types'
import { useFavorites } from '@/composables/useFavorites'

const { favorites, count, clear } = useFavorites()
const entries = ref<ApodSummary[]>([])
const loading = ref(false)

async function load() {
  const dates = favorites.value
  if (!dates.length) {
    entries.value = []
    return
  }

  loading.value = true
  const loaded: ApodSummary[] = []

  try {
    for (const date of dates) {
      try {
        const entry = await api.entry(date)
        loaded.push({
          date: entry.date,
          title: entry.title,
          media: entry.media,
          has_copyright: entry.has_copyright,
        })
      } catch {}
    }
    entries.value = loaded
  } finally {
    loading.value = false
  }
}

watch(favorites, load, { immediate: true })
</script>

<template>
  <div class="stack">
    <header class="row justify">
      <h1>Favorites</h1>
      <button v-if="count" type="button" class="chip" @click="clear">Clear all</button>
    </header>

    <p class="muted note">
      Saved in this browser only. There are no accounts, and nothing is sent to the server.
    </p>

    <p v-if="!count" class="muted empty">
      Nothing saved yet. Open an entry and press <i class="pi pi-star" aria-hidden="true" /> Save.
      <br />
      <RouterLink to="/">Start from the latest entry</RouterLink>
    </p>

    <EntryGrid v-else :entries="entries" :loading="loading" :placeholders="count" />
  </div>
</template>

<style scoped>
.justify {
  justify-content: space-between;
}

h1 {
  font-size: 1.6rem;
}

.note {
  font-size: 0.88rem;
  margin: 0;
}

.empty {
  padding: 3rem 0;
  text-align: center;
  line-height: 2.2;
}

.chip {
  font: inherit;
  font-size: 0.86rem;
  padding: 0.25rem 0.8rem;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--bg-elevated);
  color: var(--text-muted);
  cursor: pointer;
}

.chip:hover {
  color: var(--text);
}
</style>
