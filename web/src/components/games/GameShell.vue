<script lang="ts" setup>
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import type { GameSlug } from '@/api/types'
import { type GameMode, GAMES, useDailyDay, useGame } from '@/composables/useGames'

const props = defineProps<{
  slug: GameSlug
  title: string
  blurb: string
  how: string[]
  mode: GameMode | null
  day?: string
}>()

const emit = defineEmits<{ 'update:mode': [GameMode | null] }>()

const { stats, resultFor } = useGame(props.slug)
const dailyDay = useDailyDay()
const howOpen = ref(false)

const home = computed(() => GAMES.find((game) => game.slug === props.slug)?.path ?? '/games')
const played = computed(() => resultFor(dailyDay.value))
</script>

<template>
  <div :class="['stack', mode ? 'game' : 'lobby']">
    <template v-if="!mode">
      <header class="stack head">
        <RouterLink class="back muted" to="/games">
          <i aria-hidden="true" class="pi pi-angle-left" />
          All games
        </RouterLink>
        <div class="row title-row">
          <h1>{{ title }}</h1>
          <Button
            icon="pi pi-question-circle"
            label="How to play"
            severity="secondary"
            size="small"
            text
            @click="howOpen = true"
          />
        </div>
        <p class="muted blurb">{{ blurb }}</p>
      </header>

      <div class="choices">
        <button class="card choice" type="button" @click="emit('update:mode', 'daily')">
          <span class="row choice-head">
            <i aria-hidden="true" class="pi pi-calendar" />
            <strong>Today's game</strong>
            <span v-if="stats.streak" class="badge">
              <i aria-hidden="true" class="pi pi-bolt" />
              {{ stats.streak }} day{{ stats.streak === 1 ? '' : 's' }} streak
            </span>
          </span>
          <span class="muted line">
            Everybody will share the same daily game, just like Wordle >:)
          </span>
          <span :class="['state', played ? 'done' : 'open']">
            <i :class="['pi', played ? 'pi-check-circle' : 'pi-play']" aria-hidden="true" />
            {{ played ? `Played: ${played.label}` : 'Not played yet' }}
          </span>
        </button>

        <button class="card choice" type="button" @click="emit('update:mode', 'free')">
          <span class="row choice-head">
            <i aria-hidden="true" class="pi pi-sync" />
            <strong>Free play</strong>
          </span>
          <span class="muted line">
            Play as many random games as you like without touching your daily streak.
          </span>
          <span class="state open">
            <i aria-hidden="true" class="pi pi-play" />
            Always open
          </span>
        </button>
      </div>

      <GameRecord :slug="slug" :title="title" />
    </template>

    <template v-else>
      <header class="row bar">
        <RouterLink :to="home" class="back">
          <i aria-hidden="true" class="pi pi-angle-left" />
          <h1>{{ title }}</h1>
        </RouterLink>

        <span class="chip">
          <i :class="['pi', mode === 'daily' ? 'pi-calendar' : 'pi-sync']" aria-hidden="true" />
          <template v-if="mode === 'daily'">{{ day ? `Daily ${day}` : 'Daily' }}</template>
          <template v-else>Free play</template>
        </span>

        <button v-tooltip.bottom="'How to play'" class="help" type="button" @click="howOpen = true">
          <i aria-hidden="true" class="pi pi-question-circle" />
          <span class="sr-only">How to play</span>
        </button>
      </header>

      <slot />
    </template>

    <Dialog
      v-model:visible="howOpen"
      :header="`How to play ${title}`"
      :style="{ width: 'min(34rem, 94vw)' }"
      dismissable-mask
      modal
    >
      <ul class="rules">
        <li v-for="line in how" :key="line">{{ line }}</li>
      </ul>
    </Dialog>
  </div>
</template>

<style scoped>
.head {
  gap: 0.55rem;
}

.back {
  display: inline-flex;
  align-items: center;
  gap: 0.15rem;
  text-decoration: none;
  color: inherit;
  width: fit-content;
}

.head .back {
  font-size: 0.85rem;
  color: var(--text-muted);
}

.back:hover {
  color: var(--text);
}

h1 {
  font-size: 1.6rem;
}

.title-row {
  gap: 0.75rem;
}

.blurb {
  margin: 0;
  text-wrap: pretty;
}

.choices {
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 18rem), 1fr));
}

.choice {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.45rem;
  padding: 1.1rem 1.25rem 1.15rem;
  text-align: left;
  font: inherit;
  color: inherit;
  cursor: pointer;
  transition:
    transform 0.15s ease,
    border-color 0.15s ease;
}

.choice:hover,
.choice:focus-visible {
  transform: translateY(-2px);
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
}

.choice-head {
  gap: 0.5rem;
  font-size: 1.1rem;
}

.choice-head > i {
  color: var(--accent);
}

.line {
  font-size: 0.9rem;
  text-wrap: pretty;
}

.state {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  margin-top: 0.15rem;
  font-size: 0.85rem;
}

.state i {
  font-size: 0.8em;
}

.state.open {
  color: var(--accent);
}

.state.done {
  color: var(--text-muted);
}

.badge {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  background: color-mix(in srgb, var(--accent) 15%, transparent);
  color: var(--accent);
  border-radius: 999px;
  padding: 0.05rem 0.5rem;
  font-size: 0.75rem;
}

.rules {
  margin: 0;
  padding-left: 1.1rem;
  color: var(--text-muted);
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
  text-wrap: pretty;
}

.game {
  gap: 0.9rem;
}

.bar {
  gap: 0.6rem;
}

.bar h1 {
  font-size: 1.2rem;
}

.bar .back:hover h1 {
  color: var(--accent);
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  margin-left: auto;
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 0.1rem 0.7rem;
  font-size: 0.8rem;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.chip i {
  font-size: 0.85em;
}

.help {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 2rem;
  height: 2rem;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: none;
  color: var(--text-muted);
  cursor: pointer;
}

.help:hover {
  color: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
}
</style>
