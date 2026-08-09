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
        NASA has published a picture of the sky with a short explanation every day since June 1995.
        This is an unofficial archive of all of them, aimed at displaying them in a modern way and
        offering neat little features that derive data from every entry:
        <RouterLink to="/search">search</RouterLink>
        the text,
        <RouterLink to="/archive">browse</RouterLink>
        by month,
        <RouterLink to="/stats">dig through the numbers</RouterLink>
        , or
        <RouterLink to="/games">play some minigames</RouterLink>
        .
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
  gap: 0.75rem;
  padding: 1.1rem 0.9rem 1.15rem 1.2rem;
  border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
}

.body {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  min-width: 0;
}

h2 {
  font-size: 1.05rem;
  text-wrap: balance;
}

p {
  margin: 0;
  font-size: 0.92rem;
  max-width: 72ch;
  text-wrap: pretty;
  color: var(--text-muted);
}

.close {
  flex: none;
  margin-left: auto;
}
</style>
