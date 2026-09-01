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
import { daysBetween, formatDate } from '@/utils/date'

const HOW = [
  'You start with one picture and the day it appeared on APOD.',
  'A second one comes up beside it, always at least 6 months away. You say whether it is older or newer than the one you already have.',
  'Get it right and that second picture becomes the one you measure the next against, so a run walks its own way through the archive.',
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

type Guess = 'older' | 'newer'

const settled = ref<{ pair: OrderPair; picked: Guess; a: Reveal; b: Reveal; right: boolean }>()
const results = ref<boolean[]>([])
const recorded = ref<GameResult>()

/** Pictures whose date the player has already been shown, keyed by picture token. */
const learnt = ref<Map<string, Reveal>>(new Map())

/**
 * A pick is in flight. Without this, a second click while the first is still travelling settles the
 * round twice, which skips a pair and leaves the next round carrying a picture nobody has seen.
 */
const deciding = ref(false)

/** More rounds are on their way. Two requests at once would both carry on from the same picture. */
const extending = ref(false)
/** The archive has nothing left to chain on to, so there is no point asking for more. */
const exhausted = ref(false)
/** Bumped on every deal, so rounds arriving for a run the player has left are dropped. */
let dealt = 0

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

/** An endless run also ends when the archive has nothing left to chain the next round on to. */
const ranOut = computed(
  () => endless.value && pairs.value.length > 0 && !pair.value && !extending.value,
)

const over = computed(() =>
  endless.value
    ? ranOut.value || (results.value.length > 0 && !results.value[results.value.length - 1])
    : results.value.length > 0 && results.value.length >= pairs.value.length,
)
/** A settled round keeps the board up even when the next pair has not been dealt yet. */
const playing = computed(() => !!settled.value || (!!pair.value && !over.value))

/** The date on the picture being measured against is the whole premise, so it comes first. */
const held = computed(() => (pair.value ? learnt.value.get(pair.value.a.picture) : undefined))

const bands = computed<Band[]>(() =>
  endless.value ? [] : results.value.map((right) => (right ? 0 : 4)),
)

function label(): string {
  if (!endless.value) return `${correct.value} of ${results.value.length} right`
  return best.value ? `${best.value} in a row` : 'Missed the very first one >:p'
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

/**
 * Every round has to open with the picture the round before it closed on, or the player is handed a
 * stranger where the picture they have just dated belongs. The archive deals chains that overlap by
 * one, so a seam can only come from a run held over from an older build. Everything from the seam
 * on is dropped: an endless run deals itself more, and a run that cannot is over anyway.
 */
function chained(rounds: OrderPair[], played: number): OrderPair[] {
  for (let round = Math.max(played, 1); round < rounds.length; round++) {
    if (rounds[round].a.picture !== rounds[round - 1].b.picture) return rounds.slice(0, round)
  }
  return rounds
}

async function deal(): Promise<void> {
  const mine = ++dealt
  loading.value = true
  error.value = undefined
  already.value = undefined
  recorded.value = undefined
  settled.value = undefined
  results.value = []
  pairs.value = []
  learnt.value = new Map()
  extending.value = false
  exhausted.value = false

  try {
    const puzzle =
      mode.value === 'daily'
        ? await api.games.order({ day: 'today' })
        : await api.games.order({ rounds: RESERVE })
    if (mine !== dealt) return
    day.value = puzzle.day

    const played = resultFor(puzzle.day)
    if (played) {
      already.value = played
      progress.clear(puzzleKey())
      return
    }

    const saved = progress.load(puzzleKey())
    if (saved?.pairs.length && saved.results.length < saved.pairs.length) {
      pairs.value = chained(saved.pairs, saved.results.length)
      results.value = saved.results
      keep()
      return
    }

    pairs.value = puzzle.rounds
    keep()
  } catch (thrown) {
    if (mine !== dealt) return
    error.value = thrown instanceof Error ? thrown.message : 'Failed to load the puzzle.'
  } finally {
    if (mine === dealt) loading.value = false
  }
}

/**
 * The date on the picture the round is measured against. Every round after the first already knows
 * it, because it is the picture the round before settled on, so this only fetches at the start of a
 * run or when one is picked back up out of storage.
 */
async function learn(): Promise<void> {
  const reference = pair.value?.a.picture
  if (!reference || learnt.value.has(reference)) return

  try {
    const [reveal] = await api.games.reveal([reference])
    if (reveal && pair.value?.a.picture === reference) {
      learnt.value = new Map([[reference, reveal]])
    }
  } catch {}
}

/**
 * Picking up from the last picture keeps the chain unbroken across the seam. Everything here is
 * about that seam: two requests in flight would both start from the same picture and the second
 * batch would land a stranger where the picture the player has just dated belongs, and so would a
 * batch that arrives after the run it was asked for has been dealt away.
 */
async function extend(): Promise<void> {
  if (extending.value || exhausted.value) return

  const carried = pairs.value.at(-1)?.b
  if (!carried) return

  extending.value = true
  const mine = dealt

  try {
    const more = await api.games.order({ rounds: RESERVE, from: carried.picture })
    if (mine !== dealt || pairs.value.at(-1)?.b.picture !== carried.picture) return

    const fresh = [...more.rounds]
    // The archive can hand back a chain that starts somewhere else, and a round the player is
    // about to see is not the place to find that out. Put the carried picture back on the front.
    if (fresh[0] && fresh[0].a.picture !== carried.picture) {
      if (fresh[0].b.picture === carried.picture) fresh.shift()
      else fresh[0] = { ...fresh[0], a: carried }
    }

    if (!fresh.length) {
      exhausted.value = true
      return
    }

    pairs.value = chained([...pairs.value, ...fresh], at.value)
    keep()
  } catch {
  } finally {
    if (mine === dealt) extending.value = false
  }
}

async function choose(picked: Guess): Promise<void> {
  const current = pair.value
  const known = held.value
  if (!current || !known || settled.value || deciding.value) return

  deciding.value = true
  try {
    const [b] = await api.games.reveal([current.b.picture])
    const right = picked === (b.dates[0] < known.dates[0] ? 'older' : 'newer')

    settled.value = { pair: current, picked, a: known, b, right }
    results.value = [...results.value, right]
    // The picture just placed is the one the next round is measured against.
    learnt.value = new Map([[current.b.picture, b]])
    keep()

    if (endless.value && right && pairs.value.length - at.value < 4) void extend()
  } catch (thrown) {
    error.value = thrown instanceof Error ? thrown.message : 'Failed to check the guess.'
  } finally {
    deciding.value = false
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

watch(pair, (current) => {
  // A run that has caught up with the rounds it holds asks for more rather than stopping there.
  if (!current) {
    if (endless.value && pairs.value.length && !exhausted.value) void extend()
    return
  }
  void learn()
})

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
    blurb="One picture with its date, one without. Say whether the new one appeared on APOD before or after it, then carry it forward and do it again."
    slug="order"
    title="Older or Newer"
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
          :label="over ? 'See your result' : 'Next picture'"
          size="small"
          @click="settled = undefined"
        />
      </header>

      <p v-if="!settled" class="ask">
        Is the second picture older or newer than the first picture?
      </p>
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
        <div class="side">
          <GamePicture
            :alt="settled ? settled.a.title : 'The picture you are measuring against'"
            :date="settled?.a.date"
            :full="settled?.a.media.url"
            :picture="settled ? settled.pair.a : pair.a"
          />
          <GameReveal v-if="settled" :reveal="settled.a" />
          <p v-else-if="held" class="caption">
            <span class="when">
              <i aria-hidden="true" class="pi pi-calendar" />
              {{ formatDate(held.dates[0]) }}
            </span>
          </p>
          <p v-else class="caption">
            <span class="when muted unknown">
              <i aria-hidden="true" class="pi pi-spin pi-spinner" />
              Reading its date…
            </span>
          </p>
        </div>

        <div class="side">
          <GamePicture
            :alt="settled ? settled.b.title : 'The picture to place'"
            :date="settled?.b.date"
            :full="settled?.b.media.url"
            :picture="settled ? settled.pair.b : pair.b"
            :state="settled ? (settled.right ? 'right' : 'wrong') : 'plain'"
          />
          <GameReveal v-if="settled" :reveal="settled.b" />
          <div v-else class="row choices">
            <Button
              :disabled="deciding || !held"
              icon="pi pi-angle-double-left"
              label="Older"
              outlined
              @click="choose('older')"
            />
            <Button
              :disabled="deciding || !held"
              icon="pi pi-angle-double-right"
              icon-pos="right"
              label="Newer"
              outlined
              @click="choose('newer')"
            />
          </div>
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
          ? ranOut
            ? `The archive ran out of pictures to chain on to at ${best}.`
            : best
              ? `You went ${best} deep before the miss.`
              : 'The very first one got you. Try another round to improve your high score.'
          : `${correct} of ${results.length} pictures placed right.`,
      ]"
      :replayed="!recorded"
      :share="share"
      @again="again"
    />

    <p v-else-if="extending" aria-live="polite" class="muted">Dealing more…</p>

    <p v-else-if="!loading && !pairs.length && !error" class="muted">
      The archive does not hold enough pictures for this yet.
    </p>
  </GameShell>
</template>

<style scoped>
.board {
  padding: var(--space-4) var(--space-4) var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.bar {
  position: sticky;
  top: var(--header-h);
  z-index: 2;
  gap: var(--space-3);
  font-size: var(--text-sm);
  margin: -0.9rem -1rem 0;
  padding: var(--space-3) var(--space-4);
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
  margin-right: var(--space-0);
}

.ask {
  margin: 0;
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  flex-wrap: wrap;
  font-size: var(--text-md);
}

.ask strong {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
}

.right {
  color: var(--good);
}

.wrong {
  color: var(--bad);
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
  gap: var(--space-2);
}

.side :deep(.game-picture) {
  align-self: center;
}

.caption {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-1);
  margin: 0;
}

.when {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--text-sm);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.when i {
  font-size: 0.8em;
}

.when.unknown {
  font-weight: 400;
}

/* The two answers sit under the picture they are about, which is the one being placed. */
.choices {
  justify-content: center;
  gap: var(--space-2);
  flex-wrap: nowrap;
}

.choices :deep(.p-button) {
  flex: 1 1 0;
  min-width: 0;
  justify-content: center;
}

.carried {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.carried i {
  font-size: 0.85em;
}
</style>
