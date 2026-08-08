<script lang="ts" setup>
import type { Band } from '@/composables/useGames'

const props = withDefaults(
  defineProps<{ bands: Band[]; total?: number; size?: 'small' | 'normal' }>(),
  { total: 0, size: 'normal' },
)

const LABELS = ['best', 'close', 'fair', 'wide', 'missed']

const slots = () => Math.max(props.total, props.bands.length)
</script>

<template>
  <span
    :aria-label="`Rounds so far: ${bands.length} of ${slots()}`"
    :class="['bands', size]"
    role="img"
  >
    <span
      v-for="index in slots()"
      :key="index"
      :class="['band', bands[index - 1] !== undefined ? `b${bands[index - 1]}` : 'todo']"
      :title="bands[index - 1] !== undefined ? LABELS[bands[index - 1]] : 'to come'"
    />
  </span>
</template>

<style scoped>
.bands {
  display: inline-flex;
  gap: 0.22rem;
  vertical-align: -0.1em;
}

.band {
  width: 0.85rem;
  height: 0.85rem;
  border-radius: 0.22rem;
  border: 1px solid transparent;
}

.small .band {
  width: 0.62rem;
  height: 0.62rem;
  border-radius: 0.17rem;
}

.band.todo {
  border-color: var(--border);
  background: transparent;
}

.band.b0 {
  background: #15803d;
}

.band.b1 {
  background: #4ade80;
}

.band.b2 {
  background: #facc15;
}

.band.b3 {
  background: #fb923c;
}

.band.b4 {
  background: color-mix(in srgb, var(--text) 24%, transparent);
}
</style>
