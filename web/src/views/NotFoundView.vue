<script lang="ts" setup>
import { RouterLink } from 'vue-router'

const links = [
  { to: '/', label: 'Latest entry', icon: 'pi pi-image' },
  { to: '/archive', label: 'Archive', icon: 'pi pi-calendar' },
  { to: '/random', label: 'Something random', icon: 'pi pi-sync' },
]
</script>

<template>
  <div class="card notice">
    <h1>Nothing out here</h1>
    <p class="muted">The page you were trying to open does not exist.</p>
    <div class="row links">
      <RouterLink v-for="link in links" :key="link.to" :to="link.to" class="plain">
        <Button
          :icon="link.icon"
          :label="link.label"
          outlined
          severity="secondary"
          size="small"
          tabindex="-1"
        />
      </RouterLink>
    </div>
  </div>
</template>

<style scoped>
.notice {
  padding: 4rem var(--space-7);
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
}

h1 {
  font-size: var(--text-title);
}

p {
  margin: 0;
}

code {
  font-size: 0.9em;
  background: color-mix(in srgb, var(--text) 8%, transparent);
  padding: var(--space-0) var(--space-1);
  border-radius: 4px;
}

.links {
  gap: var(--space-2);
  margin-top: var(--space-3);
  justify-content: center;
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}
</style>
