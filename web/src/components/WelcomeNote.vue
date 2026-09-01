<script lang="ts" setup>
import { RouterLink } from 'vue-router'
import { useWelcome } from '@/composables/useWelcome'

const { dismissed, dismiss } = useWelcome()
</script>

<template>
  <section v-if="!dismissed" class="card welcome">
    <div class="body">
      <h2>This is an archive of every NASA Astronomy Picture of the Day</h2>
      <p>
        NASA has published a piece of astronomy media with a short explanation every day since June
        1995. This is an unofficial archive of all of them, aimed at displaying them in a modern way
        and offering neat little features that derive data from every entry, such as:
        <RouterLink to="/search">a full-text search</RouterLink>,
        <RouterLink to="/archive">a browser</RouterLink>,
        <RouterLink to="/stats">statistics</RouterLink>,
        <RouterLink to="/resources">a resource overview</RouterLink>
        and even
        <RouterLink to="/games">some minigames</RouterLink>.
      </p>
    </div>

    <Button
      aria-label="Dismiss this message"
      class="close"
      icon="pi pi-times"
      rounded
      severity="secondary"
      text
      @click="dismiss"
    />
  </section>
</template>

<style scoped>
.welcome {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-4) var(--space-5) var(--space-5);
  border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
}

.body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

h2 {
  font-size: var(--text-md);
  text-wrap: balance;
}

p {
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
  color: var(--text-muted);
}

.close {
  flex: none;
  margin-left: auto;
}
</style>
