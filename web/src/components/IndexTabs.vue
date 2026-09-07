<script lang="ts" setup>
import { RouterLink, useRoute } from 'vue-router'

const TABS = [
  { to: '/objects', label: 'Objects' },
  { to: '/credits', label: 'Credits' },
  { to: '/pictures', label: 'Encores' },
  { to: '/resources', label: 'Resources' },
]

const route = useRoute()

function on(to: string): boolean {
  return route.path === to || route.path.startsWith(`${to}/`)
}
</script>

<template>
  <nav aria-label="Indexes" class="tabs">
    <RouterLink
      v-for="tab in TABS"
      :key="tab.to"
      :aria-current="on(tab.to) ? 'page' : undefined"
      :class="{ on: on(tab.to) }"
      :to="tab.to"
      class="tab"
    >
      {{ tab.label }}
    </RouterLink>
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
  padding: var(--space-2) var(--space-4);
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--text-muted);
  text-decoration: none;
  white-space: nowrap;
  border-left: 1px solid var(--border);
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
</style>
