<script lang="ts" setup>
import { RouterLink } from 'vue-router'

export type Tab = {
  label: string
  to?: string
  value?: string
  count?: number
}

defineProps<{
  tabs: Tab[]
  label: string
  on?: (tab: Tab) => boolean
}>()

const emit = defineEmits<{ pick: [string] }>()

function choose(tab: Tab) {
  if (tab.value !== undefined) emit('pick', tab.value)
}
</script>

<template>
  <nav :aria-label="label" class="tabs">
    <component
      :is="tab.to ? RouterLink : 'button'"
      v-for="tab in tabs"
      :key="tab.to ?? tab.value"
      :aria-current="on?.(tab) ? 'page' : undefined"
      :class="['tab', { on: on?.(tab) }]"
      :to="tab.to"
      :type="tab.to ? undefined : 'button'"
      @click="choose(tab)"
    >
      {{ tab.label }}
      <span v-if="tab.count !== undefined" class="count">{{ tab.count }}</span>
    </component>
  </nav>
</template>

<style scoped>
.tabs {
  display: flex;
  align-self: flex-start;
  max-width: 100%;
  overflow-x: auto;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
}

.tab {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-3);
  font: inherit;
  font-size: var(--text-xs);
  font-weight: 600;
  color: var(--text-muted);
  text-decoration: none;
  white-space: nowrap;
  border: 0;
  border-left: 1px solid var(--border);
  background: none;
  cursor: pointer;
  transition: color var(--dur-fast) var(--ease-out);
}

.tab:first-child {
  border-left: 0;
}

.tab:hover {
  color: var(--text);
}

.tab.on {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 10%, transparent);
}

.count {
  font-weight: 400;
  font-size: var(--text-2xs);
  font-variant-numeric: tabular-nums;
  opacity: 0.75;
}
</style>
