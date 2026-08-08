<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import type { OrderPair, Reveal } from '@/api/types'
import { api } from '@/api/client'
import {
  type Band,
  type GameResult,
  shareText,
  useGame,
  useGameMode,
  useProgress,
} from '@/composables/useGames'
import { daysBetween } from '@/utils/date'

const HOW = [
  'You are prompted with two random pictures which always appeared at least 6 months apart.',
  'You have to choose the one that appeared earlier.',
  'The daily is ten rounds and you play all ten, trying to get as many of them right.',
  'Free play is an endless run: it keeps going until you get one wrong, and your score is how far you got.',
]

const RESERVE = 10

interface Saved {
  pairs: OrderPair[]
  results: boolean[]
}

const mode = useGameMode()
const loading = ref(false)
const error = ref<string>()

const day = ref<string>()
const pairs = ref<OrderPair[]>([])
const already = ref<GameResult>()

const settled = ref<{ pair: OrderPair; picked: 'a' | 'b'; a: Reveal; b: Reveal; right: boolean }>()
const results = ref<boolean[]>([])
const recorded = ref<GameResult>()

const { record, resultFor } = useGame('order')
const progress = useProgress<Saved>('order')

const at = computed(() => results.value.length)
const pair = computed(() => pairs.value[at.value])
const showing = computed(() => Math.min(at.value + (settled.value ? 0 : 1), pairs.value.length))
const endless = computed(() => !day.value)
const correct = computed(() => results.value.filter(Boolean).length)

const streak = computed(() => {
  let run = 0
  for (const right of results.value) run = right ? run + 1 : 0
  return run
})
const best = computed(() => {
  let run = 0
  let top = 0
  for (const right of results.value) {
    run = right ? run + 1 : 0
    top = Math.max(top, run)
  }
  return top
})

const over = computed(() =>
  endless.value
    ? results.value.length > 0 && !results.value[results.value.length - 1]
    : results.value.length > 0 && results.value.length >= pairs.value.length,
)
const playing = computed(() => !!pair.value && (!over.value || !!settled.value))

const bands = computed<Band[]>(() =>
  endless.value ? [] : results.value.map((right) => (right ? 0 : 4)),
)

function label(): string {
  if (!endless.value) return `${correct.value} of ${results.value.length} right`
  return best.value ? `${best.value} in a row` : 'Failed on the first pair >:p'
}

function outcome(): GameResult {
  return {
    id: '',
    at: '',
    day: day.value,
    score: endless.value ? best.value : correct.value,
    label: already.value?.label ?? label(),
    bands: already.value?.bands ?? bands.value,
  }
}

const share = computed(() => shareText('order', outcome()))

function puzzleKey(): string {
  return day.value ? `d:${day.value}` : 'f'
}

function keep(): void {
  if (!pairs.value.length || over.value) return
  progress.save(puzzleKey(), { pairs: pairs.value, results: results.value })
}

async function deal(): Promise<void> {
  loading.value = true
  error.value = undefined
  already.value = undefined
  recorded.value = undefined
  settled.value = undefined
  results.value = []
  pairs.value = []

  try {
    const puzzle =
      mode.value === 'daily'
        ? await api.games.order({ day: 'today' })
        : await api.games.order({ rounds: RESERVE })
    day.value = puzzle.day

    const played = resultFor(puzzle.day)
    if (played) {
      already.value = played
      progress.clear(puzzleKey())
      return
    }

    const held = progress.load(puzzleKey())
    if (held?.pairs.length && held.results.length < held.pairs.length) {
      pairs.value = held.pairs
      results.value = held.results
      return
    }

    pairs.value = puzzle.rounds
    keep()
  } catch (thrown) {
    error.value = thrown instanceof Error ? thrown.message : 'Failed to load the puzzle.'
  } finally {
    loading.value = false
  }
}

async function extend(): Promise<void> {
  try {
    const more = await api.games.order({ rounds: RESERVE })
    pairs.value = [...pairs.value, ...more.rounds]
    keep()
  } catch {}
}

async function choose(picked: 'a' | 'b'): Promise<void> {
  const current = pair.value
  if (!current || settled.value) return

  try {
    const [a, b] = await api.games.reveal([current.a.picture, current.b.picture])
    const right = picked === (a.dates[0] <= b.dates[0] ? 'a' : 'b')

    settled.value = { pair: current, picked, a, b, right }
    results.value = [...results.value, right]
    keep()

    if (endless.value && right && pairs.value.length - at.value < 4) void extend()
  } catch (thrown) {
    error.value = thrown instanceof Error ? thrown.message : 'Failed to check the pick.'
  }
}

function again(): void {
  if (mode.value === 'daily') mode.value = 'free'
  else void deal()
}

function gap(a: Reveal, b: Reveal): string {
  const days = daysBetween(a.dates[0], b.dates[0])
  if (days < 365) {
    const months = Math.max(1, Math.round(days / 30.44))
    return `${months} month${months === 1 ? '' : 's'} apart`
  }

  const years = Math.round((days / 365.25) * 10) / 10
  return `${years} year${years === 1 ? '' : 's'} apart`
}

watch(over, (ended) => {
  if (!ended || recorded.value) return
  progress.clear(puzzleKey())

  const result = outcome()
  recorded.value = record({
    day: result.day,
    score: result.score,
    label: result.label,
    bands: result.bands,
  })
})

watch(
  mode,
  (chosen) => {
    if (chosen) void deal()
  },
  { immediate: true },
)
</script>

<template>
  <GameShell
    v-model:mode="mode"
    :day="mode === 'daily' ? day : undefined"
    :how="HOW"
    blurb="Two pictures, months or years apart. Pick the one that appeared on APOD first."
    slug="order"
    title="Which Came First"
  >
    <RetryNotice v-if="error && !pairs.length" :busy="loading" :message="error" @retry="deal" />
    <p v-if="loading" aria-live="polite" class="muted">Dealing…</p>

    <GameOutcome
      v-else-if="already"
      :bands="already.bands"
      :daily="true"
      :day="day"
      :headline="already.label"
      :share="share"
      replayed
      @again="again"
    />

    <div v-else-if="playing" class="card board">
      <header class="row bar">
        <span v-if="endless" class="count muted">
          <i aria-hidden="true" class="pi pi-bolt" />
          <template v-if="settled && !settled.right">Run ended at {{ best }}</template>
          <template v-else-if="streak">Run of {{ streak }}</template>
          <template v-else>New run</template>
        </span>
        <span v-else class="count muted"> Round {{ showing }}/{{ pairs.length }} </span>
        <GameBands v-if="!endless" :bands="bands" :total="pairs.length" size="small" />
        <Button
          v-if="settled"
          :icon="over ? 'pi pi-flag' : 'pi pi-arrow-right'"
          :icon-pos="over ? 'left' : 'right'"
          :label="over ? 'See your result' : 'Next pair'"
          size="small"
          @click="settled = undefined"
        />
      </header>

      <p v-if="!settled" class="ask">Which of these appeared first?</p>
      <p v-else class="ask">
        <strong :class="settled.right ? 'right' : 'wrong'">
          <i
            :class="['pi', settled.right ? 'pi-check-circle' : 'pi-times-circle']"
            aria-hidden="true"
          />
          {{ settled.right ? 'Right' : 'Wrong' }}
        </strong>
        <span class="muted">{{ gap(settled.a, settled.b) }}</span>
      </p>

      <div class="pair">
        <div v-for="side in ['a', 'b'] as const" :key="side" class="side">
          <button v-if="!settled" class="pick" type="button" @click="choose(side)">
            <GamePicture :alt="`Picture ${side.toUpperCase()}`" :picture="pair[side]" />
            <span class="label">This one appeared first</span>
          </button>

          <template v-else>
            <GamePicture
              :picture="settled.pair[side]"
              :state="settled.picked === side ? (settled.right ? 'right' : 'wrong') : 'plain'"
              alt="A picture from the archive"
            />
            <GameReveal :reveal="side === 'a' ? settled.a : settled.b" />
          </template>
        </div>
      </div>
    </div>

    <GameOutcome
      v-else-if="over"
      :bands="bands"
      :daily="!!day"
      :day="day"
      :headline="label()"
      :lines="[
        endless
          ? best
            ? `You went ${best} deep before the miss.`
            : 'The very first pair got you. Try another round to improve your high score.'
          : `${correct} of ${results.length} pairs guessed right.`,
      ]"
      :replayed="!recorded"
      :share="share"
      @again="again"
    />

    <p v-else-if="!loading && !pairs.length && !error" class="muted">
      The archive does not hold enough pictures for this yet.
    </p>
  </GameShell>
</template>

<style scoped>
.board {
  padding: 0.9rem 1rem 1.1rem;
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

.bar {
  position: sticky;
  top: var(--header-h);
  z-index: 2;
  gap: 0.75rem;
  font-size: 0.85rem;
  margin: -0.9rem -1rem 0;
  padding: 0.7rem 1rem;
  border-bottom: 1px solid var(--border);
  border-radius: var(--radius) var(--radius) 0 0;
  background: var(--bg-elevated);
}

.count {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  margin-right: auto;
}

.count i {
  font-size: 0.85em;
  margin-right: 0.15rem;
}

.ask {
  margin: 0;
  display: flex;
  align-items: baseline;
  gap: 0.6rem;
  flex-wrap: wrap;
  font-size: 1rem;
}

.ask strong {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
}

.right {
  color: #16a34a;
}

.wrong {
  color: #dc2626;
}

.pair {
  --cap: 26vh;
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 15rem), 1fr));
  align-items: stretch;
}

@media (min-width: 42rem) {
  .pair {
    --cap: 46vh;
  }
}

.side {
  display: grid;
  grid-template-rows: 1fr auto;
  gap: 0.6rem;
}

.pick {
  display: grid;
  grid-template-rows: 1fr auto;
  gap: 0.5rem;
  padding: 0;
  border: 0;
  background: none;
  color: inherit;
  font: inherit;
  cursor: pointer;
  text-align: center;
}

.pick :deep(.game-picture) {
  align-self: center;
}

.pick .label {
  font-size: 0.9rem;
  color: var(--text-muted);
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 0.3rem 0.9rem;
  align-self: center;
  transition:
    color 0.15s ease,
    border-color 0.15s ease;
}

.pick:hover .label,
.pick:focus-visible .label {
  color: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
}
</style>
