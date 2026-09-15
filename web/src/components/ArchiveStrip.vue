<script lang="ts" setup>
import { ref, watch } from 'vue'
import EntryGrid from './EntryGrid.vue'
import { api } from '@/api/client'
import type { ApodSummary, SkyStrip } from '@/api/types'

defineProps<{ strips: SkyStrip[] }>()

const open = ref<SkyStrip | null>(null)
const entries = ref<ApodSummary[]>([])
const loading = ref(false)

watch(open, async (strip) => {
  entries.value = []
  if (!strip) return

  loading.value = true
  try {
    entries.value = (await api.skyStrip(strip.id)).entries
  } catch {
    entries.value = []
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <div v-if="strips.length" class="row strips">
    <button
      v-for="strip in strips"
      :key="strip.id"
      class="chip"
      type="button"
      @click="open = strip"
    >
      <AppIcon name="images" />
      {{ strip.title }}
      <span class="count">{{ strip.entries }}</span>
    </button>
  </div>

  <Dialog
    :header="open ? `${open.title} in the archive` : ''"
    :style="{ width: 'min(38rem, 94vw)' }"
    :visible="open !== null"
    class="strip-dialog"
    dismissable-mask
    modal
    @update:visible="open = null"
  >
    <EntryGrid :entries="entries" :loading="loading" :placeholders="12" credit-lead="NASA's" />
  </Dialog>
</template>

<style scoped>
.strips {
  gap: var(--space-2);
  margin-top: auto;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  min-height: var(--space-6);
  padding: 0 var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  background: none;
  color: var(--text-muted);
  font-size: var(--text-xs);
  cursor: pointer;
}

.chip:hover,
.chip:focus-visible {
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  color: var(--text);
}

.count {
  font-variant-numeric: tabular-nums;
  opacity: 0.7;
}
</style>
