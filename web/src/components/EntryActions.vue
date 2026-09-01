<script lang="ts" setup>
import { computed } from 'vue'
import { useToast } from 'primevue/usetoast'
import { useFavorites } from '@/composables/useFavorites'
import { officialUrl, originalPath } from '@/utils/apodLinks'

const props = defineProps<{
  date: string
  title?: string
  sourceUrl?: string
}>()

const toast = useToast()
const { isFavorite, toggle } = useFavorites()

const saved = computed(() => isFavorite(props.date))

const official = computed(() =>
  props.sourceUrl ? officialUrl({ source_url: props.sourceUrl }) : null,
)

function saveToggle() {
  const wasSaved = saved.value
  toggle(props.date)
  toast.add({
    severity: wasSaved ? 'secondary' : 'success',
    summary: wasSaved ? 'Removed from favorites' : 'Saved to favorites',
    detail: props.title,
    life: 2200,
  })
}

async function share() {
  const url = `${location.origin}/${props.date}`

  if (navigator.share) {
    try {
      await navigator.share({ title: props.title, url })
      return
    } catch (thrown) {
      if (thrown instanceof DOMException && thrown.name === 'AbortError') return
    }
  }

  try {
    await navigator.clipboard.writeText(url)
    toast.add({ severity: 'success', summary: 'Link copied', detail: url, life: 2200 })
  } catch {
    toast.add({
      severity: 'warn',
      summary: 'Could not share',
      detail: 'Your browser blocked both sharing and the clipboard.',
      life: 3000,
    })
  }
}
</script>

<template>
  <div class="actions">
    <button
      :aria-label="saved ? 'Saved. Remove from favorites' : 'Save to favorites'"
      :aria-pressed="saved"
      :class="{ on: saved }"
      class="act"
      type="button"
      @click="saveToggle"
    >
      <i :class="saved ? 'pi pi-star-fill' : 'pi pi-star'" aria-hidden="true" />
      <span class="label">{{ saved ? 'Saved' : 'Save' }}</span>
    </button>

    <button class="act" type="button" @click="share">
      <i aria-hidden="true" class="pi pi-share-alt" />
      <span class="label">Share</span>
    </button>

    <a :href="originalPath(date)" aria-label="Original: the page as APOD published it" class="act">
      <i aria-hidden="true" class="pi pi-file" />
      <span class="label">Original</span>
    </a>

    <a
      v-if="official"
      :href="official"
      aria-label="Source: this entry on NASA's own site"
      class="act"
      rel="noopener"
      target="_blank"
    >
      <i aria-hidden="true" class="pi pi-external-link" />
      <span class="label">Source</span>
    </a>

    <slot />
  </div>
</template>

<style scoped>
.actions {
  display: flex;
  align-items: stretch;
  flex-wrap: wrap;
  gap: var(--space-0);
}

.actions :deep(.act) {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  flex: 1 1 auto;
  min-width: max-content;
  padding: var(--space-2) var(--space-2);
  border: 0;
  border-radius: var(--radius-md);
  background: none;
  font: inherit;
  font-size: var(--text-xs);
  line-height: 1.25;
  white-space: nowrap;
  color: var(--text-muted);
  text-decoration: none;
  cursor: pointer;
  transition:
    color 0.15s ease,
    background 0.15s ease;
}

.actions :deep(.act:hover),
.actions :deep(.act:focus-visible) {
  color: var(--text);
  background: color-mix(in srgb, var(--text) 7%, transparent);
}

.actions :deep(.act.on) {
  color: var(--accent);
}

.actions :deep(.act.on:hover),
.actions :deep(.act.on:focus-visible) {
  background: color-mix(in srgb, var(--accent) 12%, transparent);
}

.actions :deep(.act i) {
  flex: none;
  font-size: var(--text-md);
}

@media (max-width: 22rem) {
  .actions :deep(.act .label) {
    display: none;
  }
}
</style>
