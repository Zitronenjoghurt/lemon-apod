<script lang="ts" setup>
import { RouterLink } from 'vue-router'

defineProps<{
  to: string
  name: string
}>()
</script>

<template>
  <li class="card row">
    <div class="body">
      <RouterLink :to="to" class="name">{{ name }}</RouterLink>
      <span v-if="$slots.note" class="muted note"><slot name="note" /></span>
    </div>

    <div class="meta"><slot name="meta" /></div>
  </li>
</template>

<style scoped>
.row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-4);
}

.body {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.name {
  font-weight: 600;
  color: inherit;
  text-decoration: none;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.name:hover {
  color: var(--accent);
}

.note {
  font-size: var(--text-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta {
  display: flex;
  align-items: baseline;
  gap: var(--space-4);
  text-align: right;
  font-variant-numeric: tabular-nums;
}

@media (max-width: 30rem) {
  .row {
    padding: var(--space-2) var(--space-3);
    gap: var(--space-3);
  }
}
</style>
