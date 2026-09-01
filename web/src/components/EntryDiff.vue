<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import DiffText from './DiffText.vue'
import { api } from '@/api/client'
import type { ApodEntry } from '@/api/types'
import { type Change, countWords, diffWords } from '@/utils/diff'
import { formatDate } from '@/utils/date'

const props = defineProps<{
  before: string
  after: string
}>()

const pair = ref<[ApodEntry, ApodEntry]>()
const error = ref<string>()
const loading = ref(false)

async function load() {
  pair.value = undefined
  error.value = undefined
  loading.value = true

  try {
    pair.value = await Promise.all([api.entry(props.before), api.entry(props.after)])
  } catch (thrown) {
    error.value = thrown instanceof Error ? thrown.message : 'Could not load the two runs.'
  } finally {
    loading.value = false
  }
}

watch(() => [props.before, props.after], load, { immediate: true })

interface Field {
  key: string
  label: string
  changes: Change[]
}

function credit(entry: ApodEntry): string {
  return (entry.credits ?? []).map((one) => `${one.role}: ${one.text}`).join('\n')
}

function file(entry: ApodEntry): string {
  const url = entry.media.hd_url ?? entry.media.url
  return url ? url.replace(/^https?:\/\//, '') : 'nothing'
}

const fields = computed<Field[]>(() => {
  if (!pair.value) return []
  const [was, now] = pair.value

  return [
    { key: 'title', label: 'Title', before: was.title, after: now.title },
    {
      key: 'explanation',
      label: 'Explanation',
      before: was.explanation_text,
      after: now.explanation_text,
    },
    { key: 'credit', label: 'Credit', before: credit(was), after: credit(now) },
    { key: 'file', label: 'Image source', before: file(was), after: file(now) },
  ]
    .filter((field) => field.before !== field.after)
    .map((field) => ({
      key: field.key,
      label: field.label,
      changes: diffWords(field.before, field.after),
    }))
})

const tally = computed(() => {
  const all = fields.value.flatMap((field) => field.changes)
  return { added: countWords(all, 'added'), removed: countWords(all, 'removed') }
})
</script>

<template>
  <div class="diff">
    <p v-if="loading" class="muted note">Loading both entries…</p>

    <p v-else-if="error" class="muted note">{{ error }}</p>

    <template v-else-if="pair">
      <p class="muted note">
        <span class="pair">
          {{ formatDate(before) }} <i aria-hidden="true" class="pi pi-arrow-right" />
          {{ formatDate(after) }}
        </span>
        <span v-if="tally.added || tally.removed" class="tally">
          <span v-if="tally.added" class="up">+{{ tally.added }}</span>
          <span v-if="tally.removed" class="down">&minus;{{ tally.removed }}</span>
          words
        </span>
      </p>

      <div v-for="field in fields" :key="field.key" class="field">
        <p class="label">{{ field.label }}</p>
        <DiffText :changes="field.changes" />
      </div>

      <p v-if="!fields.length" class="muted note">
        Word for word the same. Only the picture file behind it moved.
      </p>
    </template>
  </div>
</template>

<style scoped>
.diff {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin-top: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--text) 3%, transparent);
}

.note {
  margin: 0;
  font-size: var(--text-xs);
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1) var(--space-4);
  align-items: baseline;
}

.pair {
  font-variant-numeric: tabular-nums;
}

.pair i {
  font-size: 0.7em;
  margin-inline: var(--space-0);
}

.tally {
  display: inline-flex;
  gap: var(--space-1);
  font-variant-numeric: tabular-nums;
}

.up {
  color: color-mix(in srgb, var(--diff-added) 80%, var(--text));
  font-weight: 600;
}

.down {
  color: color-mix(in srgb, var(--diff-removed) 80%, var(--text));
  font-weight: 600;
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.label {
  margin: 0;
  font-size: var(--text-2xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  font-weight: 600;
  color: var(--text-muted);
}
</style>
