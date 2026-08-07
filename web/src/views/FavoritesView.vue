<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import { useConfirm } from 'primevue/useconfirm'
import { useToast } from 'primevue/usetoast'
import EntryGrid from '@/components/EntryGrid.vue'
import ReadFilter from '@/components/ReadFilter.vue'
import { api } from '@/api/client'
import type { ApodSummary } from '@/api/types'
import { useFavorites } from '@/composables/useFavorites'
import { useRead } from '@/composables/useRead'

const { favorites, count, clear } = useFavorites()
const { apply, active: filtered } = useRead()
const confirm = useConfirm()
const toast = useToast()

const entries = ref<ApodSummary[]>([])
const loading = ref(false)

const shown = computed(() => apply(entries.value))
const hidden = computed(() => entries.value.length - shown.value.length)

function confirmClear() {
  confirm.require({
    header: 'Clear all favorites?',
    message: `This removes all ${count.value} saved entries from this browser. There is no undo.`,
    icon: 'pi pi-exclamation-triangle',
    rejectProps: { label: 'Cancel', severity: 'secondary', outlined: true },
    acceptProps: { label: 'Clear all', severity: 'danger' },
    accept: () => {
      const removed = count.value
      clear()
      toast.add({
        severity: 'success',
        summary: 'Favorites cleared',
        detail: `${removed} ${removed === 1 ? 'entry' : 'entries'} removed.`,
        life: 2500,
      })
    },
  })
}

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
      <Button
        v-if="count"
        label="Clear all"
        icon="pi pi-trash"
        severity="danger"
        outlined
        size="small"
        @click="confirmClear"
      />
    </header>

    <p class="muted note">
      Saved in this browser only. There are no accounts, and nothing is sent to the server.
    </p>

    <p v-if="!count" class="muted empty">
      Nothing saved yet. Open an entry and press <i class="pi pi-star" aria-hidden="true" /> Save.
      <br />
      <RouterLink to="/">Start from the latest entry</RouterLink>
    </p>

    <template v-else>
      <ReadFilter :hidden="hidden" />
      <EntryGrid
        :entries="shown"
        :loading="loading"
        :placeholders="count"
        :empty="
          filtered && hidden
            ? 'Every favorite is filtered out by the read filter.'
            : 'Nothing here.'
        "
      />
    </template>
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
</style>
