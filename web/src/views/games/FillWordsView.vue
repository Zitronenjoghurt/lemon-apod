<script lang="ts" setup>
import { computed, nextTick, ref, useTemplateRef, watch } from 'vue'
import { useConfirm } from 'primevue/useconfirm'
import {
  type ApodEntry,
  type ClozePiece,
  isHidden,
  type Reveal,
  type WordsRound,
} from '@/api/types'
import { api } from '@/api/client'
import {
  type GameResult,
  shareText,
  useGame,
  useGameMode,
  useProgress,
} from '@/composables/useGames'
import { normaliseWord, wordHash } from '@/utils/wordHash'

const HOW = [
  'You are prompted with an APOD entry where every word (except common ones) is blurred out with only its character count visible.',
  'You win by revealing the title, which is written in the same blurred out words at the top.',
  'Fewer guesses make up a better score.',
]

interface Saved {
  puzzle: WordsRound
  guesses: { word: string; hits: number }[]
}

const mode = useGameMode()
const loading = ref(false)
const error = ref<string>()

const day = ref<string>()
const puzzle = ref<WordsRound>()
const already = ref<GameResult>()
const reveal = ref<Reveal>()
const answer = ref<ApodEntry>()

const guesses = ref<{ word: string; hits: number }[]>([])
const found = ref<Map<string, string>>(new Map())
const revealed = ref<Map<string, string>>(new Map())
const entry = ref('')
const surrendered = ref(false)
const recorded = ref<GameResult>()
const notice = ref('')

const field = useTemplateRef<{ $el: HTMLElement } | null>('field')
const { record, resultFor } = useGame('words')
const progress = useProgress<Saved>('words')
const confirm = useConfirm()

const salt = computed(() => puzzle.value?.salt ?? '0')
const titleHashes = computed(() => hashesOf(puzzle.value?.title ?? []))
const textHashes = computed(() => hashesOf(puzzle.value?.text ?? []))

const given = computed(() => {
  const words = new Set<string>()
  for (const piece of [...(puzzle.value?.title ?? []), ...(puzzle.value?.text ?? [])]) {
    if (isHidden(piece)) continue
    for (const word of piece.s.split(/[^\p{L}\p{N}'-]+/u)) {
      const normalised = normaliseWord(word)
      if (normalised) words.add(normalised)
    }
  }
  return words
})

function hashesOf(pieces: ClozePiece[]): string[] {
  return pieces.filter(isHidden).map((piece) => piece.h)
}

const solved = computed(
  () => !!puzzle.value && titleHashes.value.every((hash) => found.value.has(hash)),
)
const over = computed(() => solved.value || surrendered.value)

const openBlocks = computed(() => textHashes.value.filter((hash) => found.value.has(hash)).length)
const hits = computed(() => guesses.value.filter((guess) => guess.hits > 0).length)
const titleLeft = computed(() => titleHashes.value.filter((hash) => !found.value.has(hash)).length)

const percent = computed(() =>
  textHashes.value.length ? Math.round((openBlocks.value / textHashes.value.length) * 100) : 0,
)

function label(): string {
  const count = guesses.value.length
  const word = count === 1 ? 'guess' : 'guesses'
  return solved.value ? `Solved in ${count} ${word}` : `Gave up after ${count} ${word}`
}

function outcome(): GameResult {
  return {
    id: '',
    at: '',
    day: day.value,
    score: solved.value ? Math.max(50, 1_000 - 10 * guesses.value.length) : 0,
    label: already.value?.label ?? label(),
    won: solved.value,
  }
}

const share = computed(() =>
  shareText('words', outcome(), `${percent.value}% of the text uncovered`),
)

function puzzleKey(): string {
  return day.value ? `d:${day.value}` : 'f'
}

function keep(): void {
  if (!puzzle.value || over.value) return
  progress.save(puzzleKey(), { puzzle: puzzle.value, guesses: guesses.value })
}

function replay(): void {
  const round = puzzle.value
  if (!round) return

  const all = new Set([...titleHashes.value, ...textHashes.value])
  const opened = new Map<string, string>()
  for (const guess of guesses.value) {
    const hash = wordHash(round.salt, guess.word)
    if (all.has(hash)) opened.set(hash, guess.word)
  }
  found.value = opened
}

async function deal(): Promise<void> {
  loading.value = true
  error.value = undefined
  puzzle.value = undefined
  already.value = undefined
  recorded.value = undefined
  reveal.value = undefined
  answer.value = undefined
  guesses.value = []
  found.value = new Map()
  revealed.value = new Map()
  surrendered.value = false
  entry.value = ''
  notice.value = ''

  try {
    const dealt = await api.games.words(mode.value === 'daily' ? { day: 'today' } : {})
    day.value = dealt.day

    const played = resultFor(dealt.day)
    if (played) {
      already.value = played
      progress.clear(puzzleKey())
      return
    }

    const held = progress.load(puzzleKey())
    if (held?.puzzle) {
      puzzle.value = held.puzzle
      guesses.value = held.guesses
      replay()
    } else {
      puzzle.value = dealt.rounds[0]
      keep()
    }

    await nextTick()
    field.value?.$el.focus()
  } catch (thrown) {
    error.value = thrown instanceof Error ? thrown.message : 'The puzzle could not be dealt.'
  } finally {
    loading.value = false
  }
}

function submit(): void {
  const typed = normaliseWord(entry.value)
  entry.value = ''
  notice.value = ''
  if (!typed || over.value || !puzzle.value) return

  if (given.value.has(typed)) {
    notice.value = `"${typed}" is already on the page.`
    return
  }
  if (guesses.value.some((guess) => guess.word === typed)) {
    notice.value = `You have already tried "${typed}".`
    return
  }

  const hash = wordHash(salt.value, typed)
  const hitting = [...titleHashes.value, ...textHashes.value].filter((one) => one === hash).length

  if (hitting) found.value.set(hash, typed)
  guesses.value = [...guesses.value, { word: typed, hits: hitting }]
  keep()
}

function giveUp(): void {
  confirm.require({
    header: 'Give up on this one?',
    message: 'Every word will be revealed and the game is lost. There is no going back.',
    icon: 'pi pi-exclamation-triangle',
    rejectProps: { label: 'Keep going', severity: 'secondary', outlined: true },
    acceptProps: { label: 'Give up', severity: 'danger' },
    accept: () => {
      surrendered.value = true
    },
    reject: () => field.value?.$el.focus(),
  })
}

async function showAnswer(): Promise<void> {
  if (!puzzle.value || reveal.value) return
  try {
    const [got] = await api.games.reveal([puzzle.value.picture])
    reveal.value = got

    const entry = await api.entry(got.date)
    answer.value = entry
    fillIn(entry)
  } catch {}
}

function fillIn(entry: ApodEntry): void {
  const wanted = new Set([...titleHashes.value, ...textHashes.value])
  const words = new Map<string, string>()

  for (const raw of `${entry.title} ${entry.explanation_text}`.split(/[^\p{L}\p{N}'-]+/u)) {
    const word = normaliseWord(raw)
    if (!word) continue

    const hash = wordHash(salt.value, word)
    if (wanted.has(hash) && !words.has(hash)) words.set(hash, raw)
  }

  revealed.value = words
}

function again(): void {
  if (mode.value === 'daily') mode.value = 'free'
  else void deal()
}

function shown(piece: ClozePiece): {
  text?: string
  count?: number
  width?: string
  state: string
} {
  if (!isHidden(piece)) return { text: piece.s, state: 'plain' }

  const guessed = found.value.get(piece.h)
  const written = revealed.value.get(piece.h)
  if (guessed) return { text: written ?? guessed, state: 'filled' }
  if (written) return { text: written, state: 'revealed' }

  return {
    count: piece.n,
    width: `${Math.max(piece.n, 2)}ch`,
    state: over.value ? 'missed' : 'blank',
  }
}

watch(over, (ended) => {
  if (!ended || recorded.value) return
  progress.clear(puzzleKey())

  const result = outcome()
  recorded.value = record({
    day: result.day,
    score: result.score,
    label: result.label,
    won: result.won,
  })
  void showAnswer()
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
    blurb="An APOD entry with its words blurred out. Guess them one at a time until the title gives itself away."
    slug="words"
    title="Fill the Words"
  >
    <RetryNotice v-if="error && !puzzle" :busy="loading" :message="error" @retry="deal" />
    <p v-if="loading" aria-live="polite" class="muted">Dealing…</p>

    <GameOutcome
      v-else-if="already"
      :daily="true"
      :day="day"
      :headline="already.label"
      :share="share"
      replayed
      @again="again"
    />

    <div v-else-if="puzzle" class="area">
      <div class="card board">
        <header class="bar">
          <div class="row counts">
            <span class="figure">
              <strong>{{ guesses.length }}</strong>
              <span class="muted">guessed</span>
            </span>
            <span class="figure">
              <strong>{{ hits }}</strong>
              <span class="muted">hits</span>
            </span>
            <span class="figure">
              <strong>{{ titleLeft }}</strong>
              <span class="muted">left in the title</span>
            </span>
          </div>

          <div class="bar-meter">
            <div class="row meter-head">
              <span class="muted">Uncovered</span>
              <strong>{{ percent }}%</strong>
            </div>
            <div class="track">
              <div :style="{ width: `${percent}%` }" class="fill" />
            </div>
          </div>
        </header>

        <p class="cloze title">
          <span
            v-for="(piece, index) in puzzle.title"
            :key="index"
            :class="['piece', shown(piece).state]"
            :style="{ width: shown(piece).width }"
            >{{ shown(piece).text
            }}<span v-if="shown(piece).count" class="n">{{ shown(piece).count }}</span></span
          >
        </p>

        <p class="cloze body">
          <span
            v-for="(piece, index) in puzzle.text"
            :key="index"
            :class="['piece', shown(piece).state]"
            :style="{ width: shown(piece).width }"
            >{{ shown(piece).text
            }}<span v-if="shown(piece).count" class="n">{{ shown(piece).count }}</span></span
          >
        </p>
      </div>

      <form v-if="!over" class="row entry" @submit.prevent="submit">
        <InputText
          ref="field"
          v-model="entry"
          aria-label="A word to try"
          autocapitalize="off"
          autocomplete="off"
          autocorrect="off"
          class="grow"
          placeholder="Try a word"
          spellcheck="false"
        />
        <Button icon="pi pi-arrow-right" label="Guess" type="submit" />
        <Button
          aria-label="Give up"
          class="give-up"
          icon="pi pi-flag"
          label="Give up"
          severity="secondary"
          text
          type="button"
          @click="giveUp"
        />
      </form>

      <div v-if="!over" class="stack playing">
        <p v-if="notice" aria-live="polite" class="muted notice">
          <i aria-hidden="true" class="pi pi-info-circle" />
          {{ notice }}
        </p>

        <ul v-if="guesses.length" class="tried">
          <li
            v-for="guess in [...guesses].reverse()"
            :key="guess.word"
            :class="guess.hits ? 'hit' : 'miss'"
          >
            <i :class="['pi', guess.hits ? 'pi-check' : 'pi-times']" aria-hidden="true" />
            {{ guess.word }}
            <span v-if="guess.hits > 1" class="count">{{ guess.hits }}</span>
          </li>
        </ul>
      </div>

      <GameOutcome
        v-if="over"
        :daily="!!day"
        :day="day"
        :headline="outcome().label"
        :lines="[
          solved
            ? 'The title gave itself away. Congratulations! :)'
            : 'The game was solved for you: green is what you opened, red is what you have missed.',
        ]"
        :meter="{ of: percent, label: 'Text uncovered' }"
        :replayed="!recorded"
        :share="share"
        @again="again"
      >
        <GameReveal v-if="reveal" :reveal="reveal" />
      </GameOutcome>
    </div>
  </GameShell>
</template>

<style scoped>
.area {
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.board {
  padding: 1rem 1.3rem 1.3rem;
}

.bar {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 1.5rem;
  flex-wrap: wrap;
  padding-bottom: 0.8rem;
  border-bottom: 1px solid var(--border);
  margin-bottom: 1rem;
}

.counts {
  gap: 1.2rem;
}

.figure {
  display: flex;
  align-items: baseline;
  gap: 0.3rem;
  font-size: 0.85rem;
}

.figure strong {
  font-size: 1.1rem;
  font-variant-numeric: tabular-nums;
}

.bar-meter {
  flex: 1 1 9rem;
  max-width: 15rem;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  font-size: 0.8rem;
}

.meter-head {
  justify-content: space-between;
}

.track {
  height: 0.4rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  overflow: hidden;
}

.fill {
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  transition: width 0.3s ease;
}

.cloze {
  margin: 0;
  line-height: 2.2;
}

.cloze.title {
  font-size: 1.25rem;
  font-weight: 650;
  letter-spacing: -0.01em;
  margin-bottom: 1rem;
  padding-bottom: 1rem;
  border-bottom: 1px solid var(--border);
}

.cloze.body {
  font-size: 1.02rem;
}

.piece {
  white-space: pre-wrap;
}

.piece.blank,
.piece.missed {
  display: inline-block;
  position: relative;
  height: 1.15em;
  vertical-align: -0.22em;
  border-radius: 3px;
}

.piece.blank {
  background: color-mix(in srgb, var(--text) 78%, transparent);
}

.piece.missed {
  background: color-mix(in srgb, var(--text) 16%, transparent);
}

.piece .n {
  position: absolute;
  inset: 0;
  display: grid;
  place-content: center;
  font-size: 0.62em;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--bg-elevated);
  user-select: none;
}

.piece.missed .n {
  color: var(--text-muted);
}

.piece.filled,
.piece.revealed {
  border-radius: 3px;
  padding: 0 0.12em;
  font-weight: 600;
}

.piece.filled {
  background: color-mix(in srgb, #16a34a 26%, transparent);
}

.piece.revealed {
  background: color-mix(in srgb, #dc2626 24%, transparent);
}

.playing {
  gap: 0.6rem;
}

.entry {
  position: sticky;
  bottom: 0.6rem;
  z-index: 2;
  gap: 0.5rem;
  padding: 0.5rem;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--bg) 85%, transparent);
  backdrop-filter: blur(10px);
}

.grow {
  flex: 1 1 8rem;
  min-width: 0;
}

@media (max-width: 30rem) {
  .give-up :deep(.p-button-label) {
    display: none;
  }
}

.notice {
  margin: 0;
  font-size: 0.85rem;
}

.notice i {
  margin-right: 0.25rem;
}

.tried {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
  font-size: 0.85rem;
}

.tried li {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 0.05rem 0.6rem;
}

.tried li i {
  font-size: 0.7em;
}

.tried li.hit {
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  color: var(--accent);
}

.tried li.miss {
  color: var(--text-muted);
  opacity: 0.7;
}

.count {
  font-variant-numeric: tabular-nums;
  font-size: 0.75rem;
  opacity: 0.8;
}
</style>
