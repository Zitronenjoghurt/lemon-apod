<script lang="ts" setup>
import { RouterLink } from 'vue-router'
import type { Reveal } from '@/api/types'
import { formatDate } from '@/utils/date'

defineProps<{
  reveal: Reveal
  guess?: string
  days?: number
  points?: number
}>()
</script>

<template>
  <div class="reveal">
    <p class="answer">
      <RouterLink :to="`/${reveal.date}`">{{ reveal.title }}</RouterLink>
    </p>
    <dl class="facts">
      <div>
        <dt class="muted">First appearance</dt>
        <dd>{{ formatDate(reveal.date) }}</dd>
      </div>
      <div v-if="guess">
        <dt class="muted">You said</dt>
        <dd>{{ formatDate(guess) }}</dd>
      </div>
      <div v-if="days !== undefined">
        <dt class="muted">Out by</dt>
        <dd :class="days === 0 ? 'exact' : ''">
          {{
            days === 0
              ? 'nothing at all'
              : `${days.toLocaleString()} ${days === 1 ? 'day' : 'days'}`
          }}
        </dd>
      </div>
      <div v-if="points !== undefined">
        <dt class="muted">Points</dt>
        <dd>{{ points.toLocaleString() }}</dd>
      </div>
    </dl>
  </div>
</template>

<style scoped>
.reveal {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.answer {
  margin: 0;
  font-size: 1.05rem;
  font-weight: 600;
  text-wrap: pretty;
}

.answer a {
  text-decoration: none;
}

.answer a:hover {
  text-decoration: underline;
}

.facts {
  display: flex;
  flex-wrap: wrap;
  gap: 0.25rem 1.5rem;
  margin: 0;
  font-size: 0.9rem;
}

.facts div {
  display: flex;
  flex-direction: column;
}

.facts dt {
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.facts dd {
  margin: 0;
  font-variant-numeric: tabular-nums;
}

.facts .exact {
  color: #16a34a;
  font-weight: 600;
}
</style>
