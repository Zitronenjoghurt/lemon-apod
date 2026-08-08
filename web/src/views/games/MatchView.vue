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
  'Five rounds, one pick each. Guess right to win :)',
]

const FRAME = 4 / 3

interface Saved {
  rounds: MatchRound[]
  results: boolean[]
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
const results = ref<boolean[]>([])
const recorded = ref<GameResult>()

const { record, resultFor } = useGame('match')
const progress = useProgress<Saved>('match')

const at = computed(() => results.value.length)
const showing = computed(() => Math.min(at.value + (settled.value ? 0 : 1), rounds.value.length))
const round = computed(() => rounds.value[at.value])
const shown = computed(() => settled.value?.round ?? round.value)
const correct = computed(() => results.value.filter(Boolean).length)
const over = computed(() => rounds.value.length > 0 && results.value.length >= rounds.value.length)
const playing = computed(() => !!shown.value && (!over.value || !!settled.value))
const bands = computed<Band[]>(() => results.value.map((right) => (right ? 0 : 4)))

function label(): string {
  return `${correct.value} of ${results.value.length} matched`
}

function outcome(): GameResult {
  return {
    id: '',
    at: '',
    day: day.value,
    score: correct.value,
    label: already.value?.label ?? label(),
    bands: already.value?.bands ?? bands.value,
    won: correct.value === results.value.length,
  }
}

const share = computed(() => shareText('match', outcome()))

function puzzleKey(): string {
  return day.value ? `d:${day.value}` : 'f'
}

function keep(): void {
  if (!rounds.value.length || over.value) return
  progress.save(puzzleKey(), { rounds: rounds.value, results: results.value })
}

async function deal(): Promise<void> {
  loading.value = true
  error.value = undefined
  already.value = undefined
  recorded.value = undefined
  settled.value = undefined
  results.value = []
  rounds.value = []

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
    if (held?.rounds.length && held.results.length < held.rounds.length) {
      rounds.value = held.rounds
      results.value = held.results
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

async function choose(picked: string): Promise<void> {
  const current = round.value
  if (!current || settled.value) return

  try {
    const verdict = await api.games.answer(current.round, picked)
    settled.value = { round: current, picked, right: verdict.correct, answer: verdict.answer }
    results.value = [...results.value, verdict.correct]
    keep()
  } catch (thrown) {
    error.value = thrown instanceof Error ? thrown.message : 'That pick could not be checked.'
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
    blurb="One explanation and six pictures. Pick the picture it describes."
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

          <p class="explanation prose">{{ shown.explanation }}</p>

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
            <button v-if="!settled" class="pick" type="button" @click="choose(choice.picture)">
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

.explanation {
  margin: 0;
  text-wrap: pretty;
  max-height: 32vh;
  overflow-y: auto;
  padding-right: 0.6rem;
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
