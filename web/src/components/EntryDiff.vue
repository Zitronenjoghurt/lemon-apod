<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { api } from '@/api/client'
import type { ApodEntry } from '@/api/types'
import { type Change, type ChangeKind, countWords, diffWords } from '@/utils/diff'
import { roleLabel } from '@/utils/credits'
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

interface Piece {
  kind: ChangeKind
  text: string
  tail: string
}

interface Field {
  key: string
  label: string
  changes: Change[]
  pieces: Piece[]
}

function pieces(changes: Change[]): Piece[] {
  return changes.map((change) => {
    const tail = /\s*$/.exec(change.text)?.[0] ?? ''
    return {
      kind: change.kind,
      text: change.text.slice(0, change.text.length - tail.length),
      tail,
    }
  })
}

function credit(entry: ApodEntry): string {
  return (entry.credits ?? []).map((one) => `${roleLabel(one.role)}: ${one.text}`).join('\n')
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
    .map((field) => {
      const changes = diffWords(field.before, field.after)
      return { key: field.key, label: field.label, changes, pieces: pieces(changes) }
    })
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
        <p class="text">
          <template v-for="(piece, index) in field.pieces" :key="index"
            ><del v-if="piece.kind === 'removed'">{{ piece.text }}</del
            ><ins v-else-if="piece.kind === 'added'">{{ piece.text }}</ins
            ><span v-else>{{ piece.text }}</span
            >{{ piece.tail }}</template
          >
        </p>
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
  gap: 0.7rem;
  margin-top: 0.6rem;
  padding: 0.75rem 0.9rem;
  border: 1px solid var(--border);
  border-radius: 0.7rem;
  background: color-mix(in srgb, var(--text) 3%, transparent);
}

.note {
  margin: 0;
  font-size: 0.78rem;
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem 0.9rem;
  align-items: baseline;
}

.pair {
  font-variant-numeric: tabular-nums;
}

.pair i {
  font-size: 0.7em;
  margin-inline: 0.1rem;
}

.tally {
  display: inline-flex;
  gap: 0.35rem;
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
  gap: 0.2rem;
}

.label {
  margin: 0;
  font-size: 0.68rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  font-weight: 600;
  color: var(--text-muted);
}

.text {
  margin: 0;
  font-size: 0.88rem;
  line-height: 1.55;
  text-wrap: pretty;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

ins,
del {
  border-radius: 0.2rem;
  padding: 0.02em 0.12em;
  text-decoration: none;
}

ins {
  background: color-mix(in srgb, var(--diff-added) 24%, transparent);
  box-shadow: inset 0 -0.12em color-mix(in srgb, var(--diff-added) 70%, transparent);
}

del {
  background: color-mix(in srgb, var(--diff-removed) 20%, transparent);
  text-decoration: line-through;
  text-decoration-thickness: 1px;
  opacity: 0.75;
}
</style>
