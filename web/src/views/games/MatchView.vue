<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import type { MatchRound, Reveal } from '@/api/types'
import { api } from '@/api/client'
import {
  type Band,
  type GameResult,
  shareText,
  useGame,
  useGameMode,
  useProgress,
} from '@/composables/useGames'

const HOW = [
  'You are prompted with a random APOD entry and 6 pictures where only one fits the description.',
  'The explanation starts blacked out except for its opening. Uncover it a sentence at a time.',
  'The opening sentences are free. After that a round is worth less the more of the text is out, down to half once all of it is.',
  'Five rounds, one pick each. Guess right to win :)',
]

const FRAME = 4 / 3
const PER_ROUND = 1000
/** How much of the text comes free, rounded up to whole sentences. */
const FREE_SHARE = 0.1
/** What a round is worth once the whole explanation is out. */
const FLOOR = 0.5

/** Words that end in a full stop without ending a sentence. */
const ABBREVIATIONS = new Set([
  'dr',
  'mr',
  'mrs',
  'ms',
  'st',
  'prof',
  'vs',
  'etc',
  'no',
  'fig',
  'figs',
  'univ',
  'inc',
  'jr',
  'sr',
  'mt',
  'approx',
  'ca',
  'est',
  'al',
])

interface Pick {
  right: boolean
  /** What the round paid, as a share of a full one. */
  worth: number
  /** How much of the text was showing when the pick was made. */
  share: number
}

interface Saved {
  rounds: MatchRound[]
  results: Pick[]
  opened: Record<string, number>
}

const mode = useGameMode()
const loading = ref(false)
const error = ref<string>()

const day = ref<string>()
const rounds = ref<MatchRound[]>([])
const already = ref<GameResult>()

const settled = ref<{
  round: MatchRound
  picked: string
  right: boolean
  answer: Reveal
}>()
const results = ref<Pick[]>([])
const recorded = ref<GameResult>()

/** A pick is in flight. A second click while it travels would settle the round twice. */
const deciding = ref(false)

/** How many sentences of each round's explanation are showing, keyed by round token. */
const opened = ref<Record<string, number>>({})

const { record, resultFor } = useGame('match')
const progress = useProgress<Saved>('match')

const at = computed(() => results.value.length)
const showing = computed(() => Math.min(at.value + (settled.value ? 0 : 1), rounds.value.length))
const round = computed(() => rounds.value[at.value])
const shown = computed(() => settled.value?.round ?? round.value)
const correct = computed(() => results.value.filter((pick) => pick.right).length)
const over = computed(() => rounds.value.length > 0 && results.value.length >= rounds.value.length)
const playing = computed(() => !!shown.value && (!over.value || !!settled.value))

const words = computed(() => split(shown.value?.explanation ?? ''))
/** Where each sentence ends in the word list, exclusive. */
const sentences = computed(() => sentenceEnds(words.value))

/** How many sentences come free: the fewest that reach the free share of the text. */
const freeSentences = computed(() => {
  const wanted = words.value.length * FREE_SHARE
  const reached = sentences.value.findIndex((end) => end >= wanted)
  return reached === -1 ? sentences.value.length : reached + 1
})

const openSentences = computed(() => {
  const current = shown.value
  if (!current) return 0
  return Math.min(opened.value[current.round] ?? freeSentences.value, sentences.value.length)
})

const openAt = computed(() => sentences.value[openSentences.value - 1] ?? 0)
const uncovered = computed(() => (words.value.length ? openAt.value / words.value.length : 1))
const allOut = computed(() => openSentences.value >= sentences.value.length)

/** Whatever the round opened with costs nothing, so every round starts out worth all of its points. */
const freeShare = computed(() => {
  if (!words.value.length) return 1
  return (sentences.value[freeSentences.value - 1] ?? 0) / words.value.length
})

const worth = computed(() => payout(uncovered.value, freeShare.value))

const earned = computed(() =>
  results.value.reduce(
    (sum, pick) => sum + (pick.right ? Math.round(PER_ROUND * pick.worth) : 0),
    0,
  ),
)
const most = computed(() => rounds.value.length * PER_ROUND)

const readShare = computed(() =>
  results.value.length
    ? results.value.reduce((sum, pick) => sum + pick.share, 0) / results.value.length
    : 0,
)

const bands = computed<Band[]>(() => results.value.map(band))

function split(text: string): string[] {
  return text.split(/\s+/).filter(Boolean)
}

/**
 * Word indices where a sentence ends, exclusive, always finishing on the last word. Uncovering by
 * the sentence gives the player something they can actually read, which a fixed slice of words does
 * not.
 */
function sentenceEnds(words: string[]): number[] {
  if (!words.length) return []

  const ends: number[] = []
  words.forEach((word, index) => {
    const trimmed = word.replace(/["'”’)\]]+$/u, '')
    if (!/[.!?]$/.test(trimmed)) return

    // "Dr." and initials such as "J." end a word, not a sentence.
    const stem = trimmed
      .slice(0, -1)
      .toLowerCase()
      .replace(/[^\p{L}]/gu, '')
    if (stem.length <= 1 || ABBREVIATIONS.has(stem)) return

    ends.push(index + 1)
  })

  if (ends[ends.length - 1] !== words.length) ends.push(words.length)
  return ends
}

/** Full points while only the opening is showing, sliding down to half once it all is. */
function payout(share: number, free: number): number {
  if (share <= free || free >= 1) return 1
  return 1 - (1 - FLOOR) * ((share - free) / (1 - free))
}

function band(pick: Pick): Band {
  if (!pick.right) return 4
  if (pick.worth >= 0.99) return 0
  if (pick.worth >= 0.9) return 1
  if (pick.worth >= 0.75) return 2
  return 3
}

function reveal(): void {
  const current = shown.value
  if (!current || settled.value || allOut.value) return

  opened.value = { ...opened.value, [current.round]: openSentences.value + 1 }
  keep()
}

function label(): string {
  return `${earned.value.toLocaleString()} / ${most.value.toLocaleString()} points`
}

function outcome(): GameResult {
  return {
    id: '',
    at: '',
    day: day.value,
    score: earned.value,
    label: already.value?.label ?? label(),
    bands: already.value?.bands ?? bands.value,
    won: correct.value === results.value.length,
  }
}

const share = computed(() =>
  shareText('match', outcome(), `${correct.value} of ${results.value.length} matched`),
)

function puzzleKey(): string {
  return day.value ? `d:${day.value}` : 'f'
}

function keep(): void {
  if (!rounds.value.length || over.value) return
  progress.save(puzzleKey(), {
    rounds: rounds.value,
    results: results.value,
    opened: opened.value,
  })
}

async function deal(): Promise<void> {
  loading.value = true
  error.value = undefined
  already.value = undefined
  recorded.value = undefined
  settled.value = undefined
  results.value = []
  rounds.value = []
  opened.value = {}

  try {
    const puzzle = await api.games.match(mode.value === 'daily' ? { day: 'today' } : {})
    day.value = puzzle.day

    const played = resultFor(puzzle.day)
    if (played) {
      already.value = played
      progress.clear(puzzleKey())
      return
    }

    const held = progress.load(puzzleKey())
    if (playable(held)) {
      rounds.value = held.rounds
      results.value = held.results
      opened.value = held.opened ?? {}
      return
    }

    rounds.value = puzzle.rounds
    keep()
  } catch (thrown) {
    error.value = thrown instanceof Error ? thrown.message : 'The rounds could not be dealt.'
  } finally {
    loading.value = false
  }
}

/** A run held from before the reveal mechanic existed cannot be scored, so it is not resumed. */
function playable(held: Saved | undefined): held is Saved {
  if (!held?.rounds.length || held.results.length >= held.rounds.length) return false
  return held.results.every(
    (pick) => typeof pick?.right === 'boolean' && typeof pick?.worth === 'number',
  )
}

async function choose(picked: string): Promise<void> {
  const current = round.value
  if (!current || settled.value || deciding.value) return

  const read = uncovered.value
  const paid = worth.value
  deciding.value = true

  try {
    const verdict = await api.games.answer(current.round, picked)
    settled.value = { round: current, picked, right: verdict.correct, answer: verdict.answer }
    results.value = [...results.value, { right: verdict.correct, worth: paid, share: read }]
    // Once the pick is in, the rest of the text is free to read.
    opened.value = { ...opened.value, [current.round]: sentences.value.length }
    keep()
  } catch (thrown) {
    error.value = thrown instanceof Error ? thrown.message : 'That pick could not be checked.'
  } finally {
    deciding.value = false
  }
}

function again(): void {
  if (mode.value === 'daily') mode.value = 'free'
  else void deal()
}

function state(token: string): 'plain' | 'picked' | 'right' | 'wrong' {
  if (!settled.value) return 'plain'
  if (token === settled.value.answer.picture) return 'right'
  if (token === settled.value.picked) return 'wrong'
  return 'plain'
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
    won: result.won,
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
    blurb="One blacked out explanation and six pictures. Uncover as little of the text as you can and still pick right."
    slug="match"
    title="Match the Picture"
  >
    <RetryNotice v-if="error && !rounds.length" :busy="loading" :message="error" @retry="deal" />
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
        <span class="count muted">Round {{ showing }}/{{ rounds.length }}</span>
        <GameBands :bands="bands" :total="rounds.length" size="small" />
        <span v-if="!settled" class="worth">
          Worth <strong>{{ Math.round(worth * 100) }}%</strong>
        </span>
        <span v-else class="worth settled-worth">
          <i aria-hidden="true" class="pi pi-star" />
          +{{
            (results[results.length - 1]?.right
              ? Math.round(PER_ROUND * results[results.length - 1].worth)
              : 0
            ).toLocaleString()
          }}
        </span>
      </header>

      <div class="round">
        <div class="text">
          <p v-if="settled" class="ask">
            <strong :class="settled.right ? 'right' : 'wrong'">
              <i
                :class="['pi', settled.right ? 'pi-check-circle' : 'pi-times-circle']"
                aria-hidden="true"
              />
              {{ settled.right ? 'This is the one' : 'Not this one' }}
            </strong>
          </p>
          <p v-else class="ask muted">Which picture is this about?</p>

          <p class="explanation prose">
            <span
              v-for="(word, index) in words"
              :key="index"
              :class="['bit', index < openAt ? 'open' : 'hole']"
              :style="index < openAt ? undefined : { width: `${Math.max(word.length, 2)}ch` }"
              >{{ index < openAt ? word : '' }}</span
            >
          </p>

          <div v-if="!settled" class="row uncover">
            <Button
              :disabled="allOut"
              icon="pi pi-eye"
              label="Next sentence"
              outlined
              severity="secondary"
              size="small"
              @click="reveal"
            />
            <span class="muted read">
              {{
                allOut
                  ? `All ${sentences.length} sentences are out`
                  : `${openSentences} of ${sentences.length} sentences`
              }}
            </span>
          </div>

          <div v-if="settled" class="settled">
            <GameReveal :reveal="settled.answer" />
            <Button
              :label="at < rounds.length ? 'Next explanation' : 'Finish the game'"
              icon="pi pi-arrow-right"
              icon-pos="right"
              @click="settled = undefined"
            />
          </div>
        </div>

        <ul class="choices">
          <li v-for="choice in shown.choices" :key="choice.picture">
            <button
              v-if="!settled"
              :disabled="deciding"
              class="pick"
              type="button"
              @click="choose(choice.picture)"
            >
              <GamePicture
                :frame="FRAME"
                :picture="choice"
                alt="One of the pictures to choose from"
              />
            </button>
            <GamePicture
              v-else
              :frame="FRAME"
              :picture="choice"
              :state="state(choice.picture)"
              alt="One of the pictures to choose from"
            />
          </li>
        </ul>
      </div>
    </div>

    <GameOutcome
      v-else-if="over"
      :bands="bands"
      :daily="!!day"
      :day="day"
      :headline="label()"
      :lines="[
        correct === results.length
          ? 'You got all right, smartie >;)'
          : `${results.length - correct} pictures picked wrongly.`,
        `You read ${Math.round(readShare * 100)}% of the text on average.`,
      ]"
      :replayed="!recorded"
      :share="share"
      @again="again"
    />
  </GameShell>
</template>

<style scoped>
.board {
  padding: 0.9rem 1.1rem 1.2rem;
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

.bar {
  justify-content: space-between;
  gap: 0.75rem;
  font-size: 0.85rem;
  padding-bottom: 0.7rem;
  border-bottom: 1px solid var(--border);
}

.count {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  margin-right: auto;
}

.worth {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 0.05rem 0.6rem;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.worth strong {
  color: var(--text);
}

.worth i {
  font-size: 0.8em;
  color: var(--accent);
}

.settled-worth {
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
}

.round {
  display: grid;
  gap: 1rem;
  align-items: start;
}

@media (min-width: 60rem) {
  .round {
    grid-template-columns: minmax(18rem, 27rem) minmax(0, 1fr);
    gap: 1.5rem;
  }

  .explanation {
    max-height: min(46vh, 26rem);
  }
}

.text {
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
  min-width: 0;
}

/* Laid out as wrapping flex items rather than as a run of text: the blacked out words are empty
   elements, and empty elements sitting side by side give a line nowhere to break. */
.explanation {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 0.55rem 0.32em;
  margin: 0;
  max-height: 32vh;
  overflow-y: auto;
  padding-right: 0.6rem;
}

.hole {
  display: inline-block;
  height: 0.95em;
  border-radius: 3px;
  background: color-mix(in srgb, var(--text) 14%, transparent);
}

.uncover {
  gap: 0.6rem;
}

.read {
  font-size: 0.82rem;
  font-variant-numeric: tabular-nums;
}

.ask {
  margin: 0;
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

.choices {
  --cap: 22vh;
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 0.8rem;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

@media (min-width: 34rem) {
  .choices {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}

.pick {
  display: block;
  width: 100%;
  padding: 0;
  border: 0;
  background: none;
  cursor: pointer;
  border-radius: var(--radius);
  transition: transform 0.15s ease;
}

.pick:hover,
.pick:focus-visible {
  transform: translateY(-3px);
}

.pick:disabled {
  cursor: progress;
}

.pick:disabled:hover {
  transform: none;
}

.settled {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.75rem;
  border-top: 1px solid var(--border);
  padding-top: 0.85rem;
  margin-top: 0.15rem;
}
</style>
