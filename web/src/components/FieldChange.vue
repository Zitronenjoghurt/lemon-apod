<script lang="ts" setup>
import { computed } from 'vue'
import DiffText from './DiffText.vue'
import type { FieldDivergence } from '@/api/types'
import { diffWords } from '@/utils/diff'

const props = defineProps<{ row: FieldDivergence }>()

const legacy = computed(() => props.row.legacy ?? '')
const modern = computed(() => props.row.modern ?? '')

const prose = computed(() => /\s/.test(legacy.value) && /\s/.test(modern.value))

const changes = computed(() => diffWords(legacy.value, modern.value))

const label = computed(() => props.row.field.replace(/_/g, ' '))

const sides = computed(() => [
  { key: 'legacy', host: 'apod.nasa.gov', value: legacy.value },
  { key: 'modern', host: 'science.nasa.gov', value: modern.value },
])
</script>

<template>
  <div class="change">
    <p class="field">{{ label }}</p>

    <DiffText v-if="prose" :changes="changes" />

    <ul v-else class="pair">
      <li v-for="side in sides" :key="side.key">
        <code class="where">{{ side.host }}</code>
        <span class="value">{{ side.value || '—' }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.change {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.field {
  margin: 0;
  width: fit-content;
  padding: var(--space-0) var(--space-2);
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 8%, transparent);
  font-size: var(--text-xs);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.pair {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: var(--space-2);
}

.pair li {
  display: flex;
  flex-direction: column;
  gap: var(--space-0);
  min-width: 0;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}

.where {
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.value {
  font-size: var(--text-sm);
  overflow-wrap: anywhere;
}

@media (min-width: 44rem) {
  .pair {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
