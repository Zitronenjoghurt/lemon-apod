<script setup lang="ts">
import { RouterLink } from 'vue-router'
</script>

<template>
  <div class="card notice">
    <p class="mark" aria-hidden="true">✦</p>
    <h1>Nothing out here</h1>
    <p class="muted">That page doesn't exist. Entries live at <code>/YYYY-MM-DD</code>.</p>
    <div class="row links">
      <RouterLink to="/">Latest entry</RouterLink>
      <RouterLink to="/archive">Archive</RouterLink>
      <RouterLink to="/random">Something random</RouterLink>
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
  gap: 1.25rem;
  margin-top: 0.5rem;
}
</style>
