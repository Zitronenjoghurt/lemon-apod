<script lang="ts" setup>
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import ApodCredit from '@/components/ApodCredit.vue'
import RatingHelp from '@/components/rating/RatingHelp.vue'
import RatingPicture from '@/components/rating/RatingPicture.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import type { RatingCategory, RatingOutcome } from '@/api/types'
import {
  CATEGORIES,
  CATEGORY_ICONS,
  isCategory,
  otherCategory,
  spell,
  useRatingSession,
} from '@/composables/useRating'
import { useArrowKeys } from '@/composables/useArrowKeys'

const route = useRoute()
const router = useRouter()

const category = ref<RatingCategory>(
  isCategory(route.query.category) ? route.query.category : 'beautiful',
)

const { ballot, loading, sending, error, throttled, spent, cast, ready, open, vote, reset } =
  useRatingSession(category)

const ask = computed(() => CATEGORIES[ballot.value?.category ?? category.value])
const other = computed(() => CATEGORIES[otherCategory(category.value)])
const picked = ref<RatingOutcome | null>(null)
const helpOpen = ref(false)

let flash: ReturnType<typeof setTimeout> | undefined

const clock = ref(Date.now())
let ticking: ReturnType<typeof setInterval> | undefined

watch(spent, (budget) => {
  clearInterval(ticking)
  if (!budget) return
  clock.value = Date.now()
  ticking = setInterval(() => (clock.value = Date.now()), 15_000)
})

onUnmounted(() => clearInterval(ticking))

const waitLeft = computed(() => {
  const budget = spent.value
  return budget ? Math.max(0, budget.until.getTime() - clock.value) : null
})

const over = computed(() => waitLeft.value === 0)

const cap = computed(() => {
  const budget = spent.value
  if (!budget) return ''

  const span = spell(budget.windowSecs)
  return budget.scope === 'network'
    ? `Everyone sharing your connection has used up the ${budget.allowed} votes ${span} between them.`
    : `You have used up your ${budget.allowed} votes ${span}.`
})

const opensAgain = computed(() => {
  const left = waitLeft.value
  if (left === null) return ''
  if (left === 0) return 'You can vote again now.'
  if (left < 60_000) return 'You can vote again in under a minute.'
  return `You can vote again in about ${spell(Math.round(left / 1000))}.`
})

async function choose(outcome: RatingOutcome): Promise<void> {
  if (!ready.value || sending.value) return

  picked.value = outcome
  clearTimeout(flash)
  flash = setTimeout(() => (picked.value = null), 220)

  await vote(outcome)
}

useArrowKeys({
  left: () => void choose('left'),
  right: () => void choose('right'),
  down: () => void choose('tie'),
  space: () => void choose('tie'),
})

function swap(): void {
  category.value = otherCategory(category.value)
  void router.replace({ query: { ...route.query, category: category.value } })
}

watch(category, () => void open())

onMounted(() => void open(true))
</script>

<template>
  <div class="stack vote">
    <header class="row bar">
      <RouterLink class="back" to="/rating">
        <i aria-hidden="true" class="pi pi-angle-left" />
        <h1>Best APOD Voting</h1>
      </RouterLink>

      <button
        v-tooltip.bottom="`Switch to ${other.name.toLowerCase()}`"
        class="chip"
        type="button"
        @click="swap"
      >
        <i :class="CATEGORY_ICONS[category]" aria-hidden="true" />
        {{ CATEGORIES[category].short }}
        <i aria-hidden="true" class="pi pi-sort-alt swap" />
      </button>

      <button
        v-tooltip.bottom="'How this works'"
        class="help"
        type="button"
        @click="helpOpen = true"
      >
        <i aria-hidden="true" class="pi pi-question-circle" />
        <span class="sr-only">How this works</span>
      </button>
    </header>

    <p class="ask">{{ ask.ask }}</p>

    <ApodCredit class="credit" lead="Both pictures are from NASA's" variant="banner" />

    <section v-if="spent" class="stack budget">
      <i aria-hidden="true" class="pi pi-hourglass" />
      <h2>{{ over ? 'Ready to vote' : 'Reached your voting limit' }}</h2>
      <p>{{ cap }} {{ opensAgain }}</p>
      <Button
        v-if="over"
        :loading="loading"
        icon="pi pi-refresh"
        label="Carry on voting"
        size="small"
        @click="open()"
      />
      <RouterLink class="board" to="/rating">See the results</RouterLink>
    </section>

    <template v-else>
      <RetryNotice v-if="error && !ballot" :busy="loading" :message="error" @retry="open" />
      <Message v-else-if="error" :closable="false" :severity="throttled ? 'warn' : 'error'">
        {{ error }}
      </Message>

      <div v-if="loading && !ballot" class="pair">
        <Skeleton height="30vh" width="100%" />
        <Skeleton height="30vh" width="100%" />
      </div>

      <template v-else-if="ballot">
        <div class="pair">
          <RatingPicture
            :disabled="sending"
            :side="ballot.left"
            :state="picked === 'left' ? 'picked' : picked ? 'passed' : 'plain'"
            hint="← Left"
            @pick="choose('left')"
          />
          <RatingPicture
            :disabled="sending"
            :side="ballot.right"
            :state="picked === 'right' ? 'picked' : picked ? 'passed' : 'plain'"
            hint="Right →"
            @pick="choose('right')"
          />
        </div>

        <div class="row controls">
          <Button
            :disabled="sending"
            icon="pi pi-equals"
            label="I can't decide"
            outlined
            severity="secondary"
            size="small"
            @click="choose('tie')"
          />
          <p class="muted keys"><kbd>←</kbd> <kbd>→</kbd> to pick, <kbd>space</kbd> for a tie</p>
          <p v-if="cast" class="muted tally">
            {{ cast }} vote{{ cast === 1 ? '' : 's' }} this visit
          </p>
        </div>
      </template>
    </template>

    <RatingHelp v-model:visible="helpOpen" @forgot="reset" />
  </div>
</template>

<style scoped>
.vote {
  gap: var(--space-3);
}

.bar {
  gap: var(--space-3);
  font-size: var(--text-sm);
}

.back {
  display: inline-flex;
  align-items: center;
  gap: var(--space-0);
  margin-right: auto;
  text-decoration: none;
  color: inherit;
}

.back:hover {
  color: var(--accent);
}

h1 {
  font-size: var(--text-lg);
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  background: var(--bg-elevated);
  color: inherit;
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
}

.chip:hover,
.chip:focus-visible {
  border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
  color: var(--accent);
}

.chip > i:first-child {
  color: var(--accent);
  font-size: 0.9em;
}

.swap {
  font-size: 0.75em;
  color: var(--text-muted);
}

.help {
  display: grid;
  place-items: center;
  width: 2rem;
  height: 2rem;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: none;
  color: var(--text-muted);
  font-size: var(--text-md);
  cursor: pointer;
}

.help:hover,
.help:focus-visible {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 10%, transparent);
}

.ask {
  margin: 0;
  font-size: var(--text-lg);
  font-weight: 600;
  text-wrap: balance;
}

.budget {
  gap: var(--space-2);
  align-items: center;
  padding: 2.2rem var(--space-5);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-elevated);
  text-align: center;
}

.budget > i {
  font-size: var(--text-xl);
  color: var(--accent);
}

.budget h2 {
  margin: 0;
  font-size: var(--text-md);
}

.budget p {
  margin: 0;
  max-width: 34rem;
  font-size: var(--text-sm);
  line-height: 1.55;
  text-wrap: pretty;
}

.board {
  margin-top: var(--space-1);
  font-size: var(--text-sm);
}

.credit {
  font-size: var(--text-sm);
}

.pair {
  --cap: 26vh;
  display: grid;
  gap: var(--gap);
  grid-template-columns: minmax(0, 1fr);
  align-items: start;
}

@media (min-width: 42rem) {
  .pair {
    --cap: 46vh;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

.controls {
  gap: var(--space-4);
  justify-content: center;
  flex-wrap: wrap;
}

.keys {
  margin: 0;
  font-size: var(--text-xs);
}

.tally {
  margin: 0;
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
}

kbd {
  font: inherit;
  font-size: var(--text-xs);
  border: 1px solid var(--border);
  border-bottom-width: 2px;
  border-radius: var(--radius-sm);
  padding: 0 var(--space-1);
}

@media (max-width: 42rem) {
  .keys {
    display: none;
  }

  .ask {
    font-size: var(--text-md);
  }

  .credit {
    font-size: var(--text-xs);
  }
}
</style>
