<script lang="ts" setup>
import {computed, onMounted, onUnmounted, ref, watch} from 'vue'
import {RouterLink, useRoute, useRouter} from 'vue-router'
import ApodCredit from '@/components/ApodCredit.vue'
import RatingHelp from '@/components/rating/RatingHelp.vue'
import RatingPicture from '@/components/rating/RatingPicture.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import type {RatingCategory, RatingOutcome} from '@/api/types'
import {CATEGORIES, CATEGORY_ICONS, isCategory, otherCategory, spell, useRatingSession,} from '@/composables/useRating'
import {useArrowKeys} from '@/composables/useArrowKeys'

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
          <p v-if="cast" class="muted tally">{{ cast }} vote{{ cast === 1 ? '' : 's' }} this visit</p>
        </div>
      </template>
    </template>

    <RatingHelp v-model:visible="helpOpen" @forgot="reset" />
  </div>
</template>

<style scoped>
.vote {
  gap: 0.7rem;
}

.bar {
  gap: 0.75rem;
  font-size: 0.85rem;
}

.back {
  display: inline-flex;
  align-items: center;
  gap: 0.15rem;
  margin-right: auto;
  text-decoration: none;
  color: inherit;
}

.back:hover {
  color: var(--accent);
}

h1 {
  font-size: 1.15rem;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.25rem 0.7rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg-elevated);
  color: inherit;
  font: inherit;
  font-size: 0.82rem;
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
  font-size: 1.05rem;
  cursor: pointer;
}

.help:hover,
.help:focus-visible {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 10%, transparent);
}

.ask {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
  text-wrap: balance;
}

.budget {
  gap: 0.55rem;
  align-items: center;
  padding: 2.2rem 1.2rem;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-elevated);
  text-align: center;
}

.budget > i {
  font-size: 1.6rem;
  color: var(--accent);
}

.budget h2 {
  margin: 0;
  font-size: 1.05rem;
}

.budget p {
  margin: 0;
  max-width: 34rem;
  font-size: 0.9rem;
  line-height: 1.55;
  text-wrap: pretty;
}

.board {
  margin-top: 0.3rem;
  font-size: 0.88rem;
}

.credit {
  font-size: 0.82rem;
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
  gap: 0.9rem;
  justify-content: center;
  flex-wrap: wrap;
}

.keys {
  margin: 0;
  font-size: 0.78rem;
}

.tally {
  margin: 0;
  font-size: 0.78rem;
  font-variant-numeric: tabular-nums;
}

kbd {
  font: inherit;
  font-size: 0.73rem;
  border: 1px solid var(--border);
  border-bottom-width: 2px;
  border-radius: 0.3rem;
  padding: 0 0.3rem;
}

@media (max-width: 42rem) {
  .keys {
    display: none;
  }

  .ask {
    font-size: 1.05rem;
  }

  .credit {
    font-size: 0.76rem;
  }
}
</style>
