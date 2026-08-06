<script setup lang="ts">
import { RouterLink } from 'vue-router'

const links = [
  { to: '/', label: 'Latest entry', icon: 'pi pi-image' },
  { to: '/archive', label: 'Archive', icon: 'pi pi-calendar' },
  { to: '/random', label: 'Something random', icon: 'pi pi-sync' },
]
</script>

<template>
  <div class="card notice">
    <p class="mark" aria-hidden="true">✦</p>
    <h1>Nothing out here</h1>
    <p class="muted">That page doesn't exist. Entries live at <code>/YYYY-MM-DD</code>.</p>
    <div class="row links">
      <RouterLink v-for="link in links" :key="link.to" :to="link.to" class="plain">
        <Button
          :label="link.label"
          :icon="link.icon"
          severity="secondary"
          outlined
          size="small"
          tabindex="-1"
        />
      </RouterLink>
    </div>
  </div>
</template>

<style scoped>
.notice {
  padding: 4rem 2rem;
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.75rem;
}

.mark {
  font-size: 2rem;
  color: var(--accent);
  margin: 0;
}

h1 {
  font-size: 1.5rem;
}

p {
  margin: 0;
}

code {
  font-size: 0.9em;
  background: color-mix(in srgb, var(--text) 8%, transparent);
  padding: 0.1rem 0.35rem;
  border-radius: 4px;
}

.links {
  gap: 0.6rem;
  margin-top: 0.75rem;
  justify-content: center;
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}
</style>
