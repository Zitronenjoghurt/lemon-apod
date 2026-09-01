<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import { useConfirm } from 'primevue/useconfirm'
import { useToast } from 'primevue/usetoast'
import EntryGrid from '@/components/EntryGrid.vue'
import ReadFilter from '@/components/ReadFilter.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api, ApiError } from '@/api/client'
import type { ApodSummary } from '@/api/types'
import { useFavorites } from '@/composables/useFavorites'
import { provideReadScope, useRead } from '@/composables/useRead'

const { favorites, count, clear } = useFavorites()
provideReadScope('favorites')
const { apply, active: filtered } = useRead('favorites')
const confirm = useConfirm()
const toast = useToast()

const entries = ref<ApodSummary[]>([])
const loading = ref(false)
const error = ref<string>()

const loaded = new Map<string, ApodSummary>()

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
    error.value = undefined
    return
  }

  loading.value = true
  error.value = undefined
  let failed = 0

  try {
    for (const date of dates) {
      if (loaded.has(date)) continue

      try {
        const entry = await api.entry(date)
        loaded.set(date, {
          date: entry.date,
          title: entry.title,
          media: entry.media,
          has_copyright: entry.has_copyright,
        })
      } catch (thrown) {
        if (!(thrown instanceof ApiError && thrown.notFound)) failed += 1
      }
    }

    entries.value = dates
      .map((date) => loaded.get(date))
      .filter((entry): entry is ApodSummary => entry !== undefined)

    if (failed) {
      error.value = `${failed} of ${dates.length} could not be loaded.`
    }
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
        icon="pi pi-trash"
        label="Clear all"
        outlined
        severity="danger"
        size="small"
        @click="confirmClear"
      />
    </header>

    <p class="muted note">
      Favorites are saved in your browser only. They can be backed up like other site data in the
      settings.
    </p>

    <p v-if="!count" class="muted empty">
      Nothing saved yet. Open an entry and press <i aria-hidden="true" class="pi pi-star" /> Save.
      <br />
      <RouterLink to="/">Start from the latest entry</RouterLink>
    </p>

    <template v-else>
      <ReadFilter :hidden="hidden" />
      <RetryNotice v-if="error" :busy="loading" :message="error" @retry="load" />
      <EntryGrid
        :empty="
          filtered && hidden
            ? 'Every favorite is filtered out by the read filter.'
            : 'Nothing here.'
        "
        :entries="shown"
        :loading="loading"
        :placeholders="count"
      />
    </template>
  </div>
</template>

<style scoped>
.justify {
  justify-content: space-between;
}

h1 {
  font-size: var(--text-xl);
}

.note {
  font-size: var(--text-sm);
  margin: 0;
}

.empty {
  padding: var(--space-8) 0;
  text-align: center;
  line-height: 2.2;
}
</style>
