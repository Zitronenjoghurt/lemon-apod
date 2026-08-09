<script lang="ts" setup>
import { computed, onUnmounted, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import type { GamePicture as Picture, Reveal } from '@/api/types'
import { api } from '@/api/client'
import {
  type Band,
  type GameResult,
  shareText,
  useGame,
  useGameMode,
  useProgress,
} from '@/composables/useGames'
import { clampDate, daysBetween, describeGap, formatDate } from '@/utils/date'

const STAGES = [
  { blur: 34, worth: 1, seconds: 7 },
  { blur: 20, worth: 0.9, seconds: 7 },
  { blur: 12, worth: 0.8, seconds: 7 },
  { blur: 6, worth: 0.7, seconds: 7 },
  { blur: 2.5, worth: 0.6, seconds: 7 },
  { blur: 0, worth: 0.5, seconds: 0 },
]
const GRADES = ['Bullseye', 'Very close', 'Close', 'Roughly there', 'Way off']
/** How far out a guess has to land to be worth half its points, as a share of the archive's span. */
const HALF_POINTS_SHARE = 0.08
/** How close a guess has to land for each grade, again as a share of the span. */
const GRADE_SHARES = [0.01, 0.05, 0.15]
const PER_ROUND = 1000
const DAY_MS = 86_400_000

const HOW = [
  'You are prompted with a random blurred picture that sharpens every couple of seconds.',
  'Guessing fast could yield you more points, waiting for the image to fully reveal yields you the least.',
  'How close you have to land is measured against how much archive there is, so a month out is worth the same whether the archive covers three years or thirty.',
  'Some pictures may have appeared multiple times over the years. Whichever one is closest to your guess is the one scored.',
]

interface Round {
  days: number
  points: number
  stage: number
}

interface Saved {
  first: string
  last: string
  rounds: Picture[]
  scored: Round[]
  stage: number
}

const mode = useGameMode()
const loading = ref(false)
const error = ref<string>()

const day = ref<string>()
const first = ref('1995-06-16')
const last = ref('1995-06-16')
const rounds = ref<Picture[]>([])
const already = ref<GameResult>()

const stage = ref(0)
const waited = ref(0)
const guess = ref('1995-06-16')
const locked = ref<{
  picture: Picture
  reveal: Reveal
  days: number
  points: number
  stage: number
  closest: string
}>()
const scored = ref<Round[]>([])

/** A guess is in flight. A second press while it travels would score the round twice. */
const checking = ref(false)

const { record, resultFor } = useGame('date')
const progress = useProgress<Saved>('date')
let ticker: ReturnType<typeof setInterval> | undefined

const at = computed(() => scored.value.length)
const round = computed(() => rounds.value[at.value])
const shot = computed(() => locked.value?.picture ?? round.value)
const showing = computed(() => Math.min(at.value + (locked.value ? 0 : 1), rounds.value.length))
const total = computed(() => scored.value.reduce((sum, one) => sum + one.points, 0))
const max = computed(() => rounds.value.length * PER_ROUND)
const finished = computed(
  () => rounds.value.length > 0 && scored.value.length >= rounds.value.length,
)
const playing = computed(() => rounds.value.length > 0 && (!finished.value || !!locked.value))

const sharpening = computed(() => stage.value < STAGES.length - 1)
const untilClue = computed(() => Math.max(0, STAGES[stage.value].seconds - waited.value))
const clueLeft = computed(() => {
  const seconds = STAGES[stage.value].seconds
  return seconds ? untilClue.value / seconds : 0
})

const worth = computed(() => Math.round(STAGES[stage.value].worth * 100))

const landed = computed(() => {
  const one = locked.value
  if (!one) return undefined

  const you = place(guess.value)
  const truth = place(one.closest)

  return {
    mark: band(one.days),
    ceiling: Math.round(PER_ROUND * STAGES[one.stage].worth),
    you,
    truth,
    keys: [
      { kind: 'you', label: 'You said', date: guess.value, at: you },
      { kind: 'truth', label: 'It ran', date: one.closest, at: truth },
    ].sort((a, b) => a.at - b.at),
  }
})

function place(date: string): number {
  const { from, to } = bounds.value
  const at = Math.round(Date.parse(`${date}T00:00:00Z`) / DAY_MS)
  return to > from ? Math.min(100, Math.max(0, ((at - from) / (to - from)) * 100)) : 0
}

/**
 * Grades are measured against how wide the archive is rather than against a fixed number of days.
 * A month out of thirty years is a good guess. A month out of one year is not, and a partial
 * archive or another twenty years of entries should not quietly change what a grade means.
 */
function band(days: number): Band {
  if (days === 0) return 0

  const share = days / span.value
  const grade = GRADE_SHARES.findIndex((edge) => share <= edge)
  return (grade === -1 ? 4 : grade + 1) as Band
}

const bands = computed(() => scored.value.map((one) => band(one.days)))
const label = computed(
  () => `${total.value.toLocaleString()} / ${max.value.toLocaleString()} points`,
)

const share = computed(() =>
  shareText('date', {
    id: '',
    at: '',
    day: day.value,
    score: total.value,
    label: already.value?.label ?? label.value,
    bands: already.value?.bands ?? bands.value,
  }),
)

const slider = computed({
  get: () => Math.round(Date.parse(`${guess.value}T00:00:00Z`) / DAY_MS),
  set: (days: number) => {
    guess.value = new Date(days * DAY_MS).toISOString().slice(0, 10)
  },
})

const bounds = computed(() => ({
  from: Math.round(Date.parse(`${first.value}T00:00:00Z`) / DAY_MS),
  to: Math.round(Date.parse(`${last.value}T00:00:00Z`) / DAY_MS),
}))

/** How many days the archive covers. Everything about how close a guess landed is relative to it. */
const span = computed(() => Math.max(1, bounds.value.to - bounds.value.from))
const halfPoints = computed(() => Math.max(1, span.value * HALF_POINTS_SHARE))

function stopTicking(): void {
  if (ticker) clearInterval(ticker)
  ticker = undefined
}

function sharpen(): void {
  if (!sharpening.value || locked.value) return

  waited.value = 0
  stage.value += 1
  keep()
  if (!sharpening.value) stopTicking()
}

function revealAll(): void {
  if (!sharpening.value || locked.value) return

  stopTicking()
  waited.value = 0
  stage.value = STAGES.length - 1
  keep()
}

function startTicking(from = 0): void {
  stopTicking()
  stage.value = from
  waited.value = 0
  if (!sharpening.value) return

  ticker = setInterval(() => {
    waited.value += 1
    if (waited.value >= STAGES[stage.value].seconds) sharpen()
  }, 1_000)
}

function middle(): string {
  const { from, to } = bounds.value
  return new Date(Math.round((from + to) / 2) * DAY_MS).toISOString().slice(0, 10)
}

function puzzleKey(): string {
  return day.value ? `d:${day.value}` : 'f'
}

function keep(): void {
  if (!rounds.value.length || finished.value) return
  progress.save(puzzleKey(), {
    first: first.value,
    last: last.value,
    rounds: rounds.value,
    scored: scored.value,
    stage: stage.value,
  })
}

async function deal(): Promise<void> {
  stopTicking()
  loading.value = true
  error.value = undefined
  locked.value = undefined
  already.value = undefined
  scored.value = []
  rounds.value = []

  try {
    const puzzle = await api.games.date(mode.value === 'daily' ? { day: 'today' } : {})
    day.value = puzzle.day
    first.value = puzzle.first
    last.value = puzzle.last
    guess.value = middle()

    const played = resultFor(puzzle.day)
    if (played) {
      already.value = played
      progress.clear(puzzleKey())
      return
    }

    const held = progress.load(puzzleKey())
    if (held && held.rounds.length && held.scored.length < held.rounds.length) {
      first.value = held.first
      last.value = held.last
      rounds.value = held.rounds
      scored.value = held.scored
      guess.value = middle()
      startTicking(held.stage)
      return
    }

    rounds.value = puzzle.rounds
    startTicking()
    keep()
  } catch (thrown) {
    error.value = thrown instanceof Error ? thrown.message : 'The puzzle could not be dealt.'
  } finally {
    loading.value = false
  }
}

function points(days: number, worth: number): number {
  return Math.round((PER_ROUND * worth) / (1 + days / halfPoints.value))
}

async function lockIn(): Promise<void> {
  const current = round.value
  if (!current || locked.value || checking.value) return

  const chosen = clampDate(guess.value, first.value, last.value)
  const held = stage.value
  stopTicking()
  checking.value = true

  try {
    const [reveal] = await api.games.reveal([current.picture])
    const closest = reveal.dates.reduce((near, date) =>
      daysBetween(chosen, date) < daysBetween(chosen, near) ? date : near,
    )
    const days = daysBetween(chosen, closest)
    const earned = points(days, STAGES[held].worth)

    guess.value = chosen
    locked.value = { picture: current, reveal, days, points: earned, stage: held, closest }
    scored.value = [...scored.value, { days, points: earned, stage: held }]
    keep()
  } catch (thrown) {
    error.value = thrown instanceof Error ? thrown.message : 'That guess could not be checked.'
    startTicking(held)
  } finally {
    checking.value = false
  }
}

function next(): void {
  locked.value = undefined
  guess.value = middle()
  if (at.value < rounds.value.length) {
    startTicking()
    keep()
  }
}

function again(): void {
  if (mode.value === 'daily') mode.value = 'free'
  else void deal()
}

watch(finished, (over) => {
  if (!over) return
  progress.clear(puzzleKey())
  record({ day: day.value, score: total.value, label: label.value, bands: bands.value })
})

watch(
  mode,
  (chosen) => {
    stopTicking()
    if (chosen) void deal()
  },
  { immediate: true },
)

onUnmounted(stopTicking)
</script>

<template>
  <GameShell
    v-model:mode="mode"
    :day="mode === 'daily' ? day : undefined"
    :how="HOW"
    blurb="A random blurred picture that sharpens slowly. Guess the date it appeared as fast as possible."
    slug="date"
    title="Guess the Date"
  >
    <RetryNotice v-if="error && !playing" :busy="loading" :message="error" @retry="deal" />
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
        <GameBands :bands="bands" :total="rounds.length" />
        <span class="score">{{ total.toLocaleString() }}<span class="muted"> pts</span></span>
      </header>

      <div class="stage">
        <GamePicture
          v-if="shot"
          :blur="locked ? 0 : STAGES[stage].blur"
          :picture="shot"
          alt="The picture to date"
        />

        <div v-if="!locked" class="controls">
          <div class="clue">
            <div class="row clue-head">
              <span class="worth">
                Worth <strong>{{ worth }}%</strong>
              </span>

              <span v-if="sharpening" class="muted next">
                sharper in <strong class="secs">{{ untilClue }}s</strong>
              </span>
              <span v-else class="muted next">fully in focus</span>

              <template v-if="sharpening">
                <Button
                  icon="pi pi-bolt"
                  label="Sharpen"
                  severity="secondary"
                  size="small"
                  text
                  @click="sharpen"
                />
                <Button
                  icon="pi pi-eye"
                  label="Full focus"
                  severity="secondary"
                  size="small"
                  text
                  @click="revealAll"
                />
              </template>
            </div>
            <div class="track thin">
              <div :style="{ width: `${clueLeft * 100}%` }" class="fill wait" />
            </div>
          </div>

          <div class="timeline">
            <input
              v-model.number="slider"
              :max="bounds.to"
              :min="bounds.from"
              aria-label="Slide to the date you want to guess"
              step="1"
              type="range"
            />
            <div class="ends muted">
              <span>{{ first }}</span>
              <span>{{ last }}</span>
            </div>
          </div>

          <div class="row picker">
            <input
              v-model="guess"
              :max="last"
              :min="first"
              aria-label="Your guess, as a date"
              class="date-input"
              type="date"
            />
            <Button :loading="checking" icon="pi pi-check" label="Lock it in" @click="lockIn" />
          </div>
        </div>

        <div v-else-if="landed" :class="['settled', `b${landed.mark}`]">
          <div class="score-head">
            <span class="grade">{{ GRADES[landed.mark] }}</span>
            <span class="muted gap">{{ describeGap(locked.days) }}</span>
            <strong class="earned">
              +{{ locked.points.toLocaleString() }}
              <span class="muted of">of {{ landed.ceiling.toLocaleString() }}</span>
            </strong>
          </div>

          <div class="landed">
            <div class="scale">
              <span
                :style="{
                  left: `${Math.min(landed.you, landed.truth)}%`,
                  width: `${Math.abs(landed.truth - landed.you)}%`,
                }"
                class="span"
              />
              <span :style="{ left: `${landed.you}%` }" class="pin you" />
              <span :style="{ left: `${landed.truth}%` }" class="pin truth" />
            </div>
            <div class="ends muted">
              <span>{{ first }}</span>
              <span>{{ last }}</span>
            </div>
            <p class="legend">
              <span v-for="key in landed.keys" :key="key.kind" :class="['key', key.kind]">
                {{ key.label }} {{ formatDate(key.date) }}
              </span>
            </p>
          </div>

          <p class="answer">
            <RouterLink :to="`/${locked.reveal.date}`">{{ locked.reveal.title }}</RouterLink>
            <span v-if="locked.reveal.dates.length > 1" class="muted reruns">
              Appeared {{ locked.reveal.dates.length }} times. The nearest one counted.
            </span>
          </p>

          <Button
            :label="at < rounds.length ? 'Next picture' : 'See the score'"
            icon="pi pi-arrow-right"
            icon-pos="right"
            @click="next"
          />
        </div>
      </div>
    </div>

    <GameOutcome
      v-else-if="finished"
      :bands="bands"
      :daily="!!day"
      :day="day"
      :headline="label"
      :lines="[
        `Your closest guess was ${describeGap(Math.min(...scored.map((one) => one.days)))}, the furthest ${describeGap(Math.max(...scored.map((one) => one.days)))}.`,
      ]"
      :share="share"
      @again="again"
    >
      <table class="rounds">
        <tbody>
          <tr v-for="(one, index) in scored" :key="index">
            <td class="muted">{{ index + 1 }}</td>
            <td>
              <GameBands :bands="[band(one.days)]" size="small" />
            </td>
            <td class="gap">{{ describeGap(one.days) }}</td>
            <td class="pts">{{ one.points.toLocaleString() }}</td>
          </tr>
        </tbody>
      </table>
    </GameOutcome>
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
  justify-content: space-between;
  gap: 0.75rem;
  font-size: 0.85rem;
  padding-bottom: 0.7rem;
  border-bottom: 1px solid var(--border);
}

.count,
.score {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.score {
  font-weight: 600;
}

.stage {
  --cap: 32vh;
  display: grid;
  gap: 0.9rem;
  align-items: start;
}

@media (min-width: 58rem) {
  .stage {
    --cap: 58vh;
    grid-template-columns: minmax(0, 1.05fr) minmax(18rem, 0.95fr);
    gap: 1.25rem;
    align-items: center;
  }
}

.controls,
.settled {
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

/* The reveal fills the column the controls it replaced filled, so the strip and the score line up
   with the meters that were there a moment ago. Only the button keeps to its own width. */
.settled {
  gap: 0.7rem;
}

.settled :deep(.p-button) {
  align-self: flex-start;
}

.clue {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  font-size: 0.85rem;
}

.clue-head {
  gap: 0.35rem 0.6rem;
  min-height: 2rem;
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

.next {
  margin-right: auto;
  white-space: nowrap;
}

.secs {
  font-variant-numeric: tabular-nums;
  color: var(--text);
}

.clue-head :deep(.p-button) {
  padding-block: 0.15rem;
}

.track {
  height: 0.4rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  overflow: hidden;
}

.track.thin {
  height: 0.25rem;
}

.fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  transition: width 0.6s ease;
}

.fill.wait {
  background: color-mix(in srgb, var(--text) 35%, transparent);
  transition: width 1s linear;
}

.small {
  font-size: 0.75rem;
}

.score-head {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.grade {
  font-weight: 650;
  color: var(--mark);
  background: color-mix(in srgb, var(--mark) 16%, transparent);
  border-radius: 999px;
  padding: 0.1rem 0.6rem;
}

.gap {
  font-size: 0.9rem;
}

.earned {
  display: inline-flex;
  align-items: baseline;
  gap: 0.35rem;
  font-size: 1.4rem;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.02em;
  margin-left: auto;
}

.of {
  font-size: 0.75rem;
  font-weight: 400;
  letter-spacing: 0;
}

.b0 {
  --mark: #15803d;
}

.b1 {
  --mark: #4ade80;
}

.b2 {
  --mark: #facc15;
}

.b3 {
  --mark: #fb923c;
}

.b4 {
  --mark: color-mix(in srgb, var(--text) 45%, transparent);
}

.answer {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
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

.reruns {
  font-size: 0.78rem;
  font-weight: 400;
}

.landed {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.scale {
  position: relative;
  height: 1.1rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.span {
  position: absolute;
  top: 50%;
  height: 0.28rem;
  min-width: 1px;
  transform: translateY(-50%);
  border-radius: 999px;
  background: var(--mark);
  opacity: 0.75;
}

.pin {
  position: absolute;
  top: 50%;
  width: 0.62rem;
  height: 0.62rem;
  margin-left: -0.31rem;
  transform: translateY(-50%);
  border-radius: 999px;
}

.pin.you {
  background: var(--bg-elevated);
  border: 2px solid var(--text-muted);
}

.pin.truth {
  background: #16a34a;
  box-shadow: 0 0 0 2px color-mix(in srgb, #16a34a 25%, transparent);
}

.legend {
  display: flex;
  flex-wrap: wrap;
  gap: 0.2rem 1rem;
  margin: 0;
  font-size: 0.8rem;
  font-variant-numeric: tabular-nums;
}

.key {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  color: var(--text-muted);
}

.key::before {
  content: '';
  width: 0.55rem;
  height: 0.55rem;
  border-radius: 999px;
}

.key.you::before {
  background: var(--bg-elevated);
  border: 2px solid var(--text-muted);
}

.key.truth::before {
  background: #16a34a;
}

.timeline input[type='range'] {
  width: 100%;
  accent-color: var(--accent);
}

.ends {
  display: flex;
  justify-content: space-between;
  font-size: 0.75rem;
  font-variant-numeric: tabular-nums;
}

.picker {
  gap: 0.6rem;
}

.date-input {
  font: inherit;
  color: var(--text);
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 0.6rem;
  padding: 0.5rem 0.7rem;
  font-variant-numeric: tabular-nums;
}

.rounds {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9rem;
  margin-top: 0.3rem;
}

.rounds td {
  padding: 0.3rem 0;
  border-top: 1px solid var(--border);
  vertical-align: middle;
}

.rounds td:first-child {
  width: 1%;
  padding-right: 0.6rem;
  font-variant-numeric: tabular-nums;
}

.rounds td:nth-child(2) {
  width: 1%;
  padding-right: 0.7rem;
}

.pts {
  text-align: right;
  font-variant-numeric: tabular-nums;
}
</style>
