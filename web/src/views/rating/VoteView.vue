<script lang="ts" setup>
import { computed, onMounted, onUnmounted, ref, useTemplateRef, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import ApodCredit from '@/components/ApodCredit.vue'
import MediaLightbox from '@/components/MediaLightbox.vue'
import type { Slide } from '@/components/MediaLightbox.vue'
import RatingHelp from '@/components/rating/RatingHelp.vue'
import RatingPicture from '@/components/rating/RatingPicture.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import type { BallotSide, RatingCategory, RatingOutcome } from '@/api/types'
import { measure } from '@/utils/image'
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

type Seat = 'left' | 'right'

const zoomAt = ref<number | null>(null)
const slides = ref<Slide[]>([])
const measuring = ref<Seat | null>(null)

const leftCard = useTemplateRef<InstanceType<typeof RatingPicture>>('leftCard')
const rightCard = useTemplateRef<InstanceType<typeof RatingPicture>>('rightCard')

function slideFor(
  side: BallotSide,
  size: { width: number; height: number },
  from: () => HTMLImageElement | null,
): Slide | null {
  const file = side.media.url
  if (!file || !size.width) return null

  const big = side.media.hd_url
  return {
    src: file,
    width: size.width,
    height: size.height,
    alt: side.title,
    hd: big && big !== file ? big : undefined,
    thumb: side.media.thumb_url ?? undefined,
    entry: `/${side.date}`,
    source: side.source_url,
    credit: side.credit,
    from,
  }
}

async function zoom(seat: Seat): Promise<void> {
  const pair = ballot.value
  if (!pair || measuring.value !== null) return

  measuring.value = seat
  const [leftSize, rightSize] = await Promise.all([
    measure(pair.left.media.url ?? ''),
    measure(pair.right.media.url ?? ''),
  ])
  measuring.value = null
  if (ballot.value !== pair) return

  const built = [
    {
      seat: 'left' as const,
      slide: slideFor(pair.left, leftSize, () => leftCard.value?.picture ?? null),
    },
    {
      seat: 'right' as const,
      slide: slideFor(pair.right, rightSize, () => rightCard.value?.picture ?? null),
    },
  ].flatMap((one) => (one.slide ? [{ seat: one.seat, slide: one.slide }] : []))

  if (!built.length) return

  const at = built.findIndex((one) => one.seat === seat)
  slides.value = built.map((one) => one.slide)
  zoomAt.value = at === -1 ? 0 : at
}

watch(ballot, () => (zoomAt.value = null))

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
  up: () => void choose('tie'),
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
        <AppIcon name="chevron-left" />
        <h1>Best APOD Voting</h1>
      </RouterLink>

      <button
        v-tooltip.bottom="`Switch to ${other.name.toLowerCase()}`"
        class="chip"
        type="button"
        @click="swap"
      >
        <AppIcon :name="CATEGORY_ICONS[category]" />
        {{ CATEGORIES[category].short }}
        <AppIcon name="swap" class="swap" />
      </button>

      <p class="muted keys"><kbd>&larr;</kbd><kbd>&rarr;</kbd> pick <kbd>space</kbd> draw</p>

      <span v-if="cast" class="tally"> {{ cast }} vote{{ cast === 1 ? '' : 's' }} </span>

      <button
        v-tooltip.bottom="'How this works'"
        class="help"
        type="button"
        @click="helpOpen = true"
      >
        <AppIcon name="question" />
        <span class="sr-only">How this works</span>
      </button>
    </header>

    <p class="ask">{{ ask.ask }}</p>

    <ApodCredit class="credit" lead="Both pictures are from NASA's" variant="banner" />

    <section v-if="spent" class="stack budget">
      <AppIcon name="hourglass" />
      <h2>{{ over ? 'Ready to vote' : 'Reached your voting limit' }}</h2>
      <p>{{ cap }} {{ opensAgain }}</p>
      <Button v-if="over" :loading="loading" label="Carry on voting" size="small" @click="open()">
        <template #icon><AppIcon name="refresh" /></template>
      </Button>
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
            ref="leftCard"
            :busy="measuring === 'left'"
            :disabled="sending"
            :side="ballot.left"
            :state="picked === 'left' ? 'picked' : picked ? 'passed' : 'plain'"
            @pick="choose('left')"
            @zoom="zoom('left')"
          />

          <button
            v-tooltip.top="'Neither, or both equally'"
            :disabled="sending"
            class="draw"
            type="button"
            @click="choose('tie')"
          >
            <AppIcon name="equals" />
            <span class="word">It's a draw</span>
          </button>

          <RatingPicture
            ref="rightCard"
            :busy="measuring === 'right'"
            :disabled="sending"
            :side="ballot.right"
            :state="picked === 'right' ? 'picked' : picked ? 'passed' : 'plain'"
            @pick="choose('right')"
            @zoom="zoom('right')"
          />
        </div>

        <MediaLightbox :at="zoomAt" :slides="slides" @close="zoomAt = null" />
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

.budget > .icon {
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
  --cap: 30vh;
  display: grid;
  gap: var(--space-2);
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: auto auto auto;
  align-items: start;
  justify-items: center;
}

@media (min-width: 42rem) {
  .pair {
    --cap: 52vh;
    gap: var(--space-3);
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    grid-template-rows: auto;
    align-items: center;
  }
}

.draw {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  width: 100%;
  min-height: 2.25rem;
  padding: 0 var(--space-3);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-elevated);
  color: var(--text-muted);
  font: inherit;
  font-size: var(--text-xs);
  cursor: pointer;
  transition:
    color var(--dur-fast) var(--ease-out),
    border-color var(--dur-fast) var(--ease-out);
}

.draw:hover:not(:disabled),
.draw:focus-visible {
  color: var(--text);
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
}

.draw:disabled {
  cursor: default;
}

@media (min-width: 42rem) {
  .draw {
    flex-direction: column;
    width: 2.75rem;
    min-height: 2.75rem;
    padding: var(--space-2) 0;
    border-radius: var(--radius-pill);
    gap: var(--space-0);
  }

  .draw .word {
    display: none;
  }
}

.tally {
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
  color: var(--text-muted);
  white-space: nowrap;
}

.keys {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  margin: 0;
  font-size: var(--text-2xs);
  white-space: nowrap;
}

kbd {
  font: inherit;
  min-width: 1.2rem;
  padding: 0 var(--space-1);
  border: 1px solid var(--border);
  border-bottom-width: 2px;
  border-radius: var(--radius-sm);
  text-align: center;
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
