<script lang="ts" setup>
import { RouterLink } from 'vue-router'
import { useGameSummary } from '@/composables/useGames'

const games = useGameSummary()
</script>

<template>
  <div class="stack">
    <header class="stack head">
      <h1>Games</h1>
    </header>

    <ul class="grid games">
      <li v-for="game in games" :key="game.slug">
        <RouterLink :to="game.path" class="card game">
          <span class="row title">
            <i :class="game.icon" aria-hidden="true" />
            <strong>{{ game.name }}</strong>
          </span>
          <span class="muted blurb">{{ game.blurb }}</span>

          <span class="row foot">
            <span v-if="game.streak" class="badge">
              <i aria-hidden="true" class="pi pi-bolt" />
              {{ game.streak }} day{{ game.streak === 1 ? '' : 's' }} streak
            </span>
            <span :class="['today', game.today ? 'muted' : 'open']">
              <i :class="['pi', game.today ? 'pi-check-circle' : 'pi-play']" aria-hidden="true" />
              {{ game.today ? "Today's is done" : "Today's is waiting" }}
            </span>
          </span>
        </RouterLink>
      </li>
    </ul>

    <p class="muted note">
      Scores, streaks and history are kept locally in your browser. You won't need an account,
      though you might lose your history if you dont back it up regularly in the settings.
    </p>
  </div>
</template>

<style scoped>
.head {
  gap: 0.6rem;
}

h1 {
  font-size: 1.6rem;
}

.games {
  list-style: none;
  margin: 0;
  padding: 0;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 19rem), 1fr));
}

.game {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  height: 100%;
  padding: 1.1rem 1.25rem;
  text-decoration: none;
  color: inherit;
  transition:
    transform 0.15s ease,
    border-color 0.15s ease;
}

.game:hover {
  transform: translateY(-2px);
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
}

.title {
  gap: 0.5rem;
  font-size: 1.05rem;
}

.title i {
  color: var(--accent);
}

.game .blurb {
  flex: 1 1 auto;
  font-size: 0.9rem;
  text-wrap: pretty;
}

.foot {
  gap: 0.6rem;
  font-size: 0.8rem;
}

.badge {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  background: color-mix(in srgb, var(--accent) 15%, transparent);
  color: var(--accent);
  border-radius: 999px;
  padding: 0.05rem 0.5rem;
}

.today {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
}

.today i {
  font-size: 0.85em;
}

.today.open {
  color: var(--accent);
}

.note {
  font-size: 0.85rem;
  margin: 0;
  text-wrap: pretty;
}
</style>
