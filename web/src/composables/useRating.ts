import {computed, ref, type Ref, shallowRef} from 'vue'
import {api, ApiError, type SpentBudget} from '@/api/client'
import type {Ballot, RatingCategory, RatingOutcome} from '@/api/types'

export const CARD_KEY = 'apod:rating-card'
const HELD_KEY = 'apod:rating-ballot'

export const CATEGORIES: Record<RatingCategory, { name: string; short: string; ask: string }> = {
  beautiful: {
    name: 'Most beautiful',
    short: 'Beautiful',
    ask: 'Which one would you rather put on a wall?',
  },
  fascinating: {
    name: 'Most fascinating',
    short: 'Fascinating',
    ask: "Which one makes you want to learn more about what you're looking at?",
  },
}

export const CATEGORY_ICONS: Record<RatingCategory, string> = {
  beautiful: 'pi pi-heart',
  fascinating: 'pi pi-sparkles',
}

export const ORDER: RatingCategory[] = ['beautiful', 'fascinating']

export function spell(seconds: number): string {
  const minutes = Math.max(1, Math.round(seconds / 60))
  if (minutes % 60 === 0) {
    const hours = minutes / 60
    return hours === 1 ? 'an hour' : `${hours} hours`
  }
  return minutes === 1 ? 'a minute' : `${minutes} minutes`
}

export function isCategory(raw: unknown): raw is RatingCategory {
  return raw === 'beautiful' || raw === 'fascinating'
}

export function otherCategory(category: RatingCategory): RatingCategory {
  return category === 'beautiful' ? 'fascinating' : 'beautiful'
}

function prefetch(ballot: Ballot | null): void {
  if (!ballot) return
  for (const side of [ballot.left, ballot.right]) {
    const url = side.media.thumb_url
    if (!url) continue
    const image = new Image()
    image.decoding = 'async'
    image.src = url
  }
}

function hold(ballot: Ballot | null): void {
  try {
    if (!ballot) {
      sessionStorage.removeItem(HELD_KEY)
      return
    }
    const expires = Date.now() + ballot.life * 1_000
    sessionStorage.setItem(HELD_KEY, JSON.stringify({ expires, ballot }))
  } catch {}
}

function held(category: RatingCategory): Ballot | null {
  try {
    const raw = sessionStorage.getItem(HELD_KEY)
    if (!raw) return null

    const saved = JSON.parse(raw) as { expires?: number; ballot?: Ballot }
    const ballot = saved.ballot
    if (!ballot?.ballot || ballot.category !== category) return null
    if (!saved.expires || saved.expires <= Date.now()) return null

    return ballot
  } catch {
    return null
  }
}

export function useRatingSession(category: Ref<RatingCategory>) {
  const ballot = shallowRef<Ballot | null>(null)
  const loading = ref(false)
  const sending = ref(false)
  const error = ref<string>()
  const throttled = ref(false)
  const spent = ref<SpentBudget | null>(null)
  const cast = ref(0)
  let lastVote = 0

  const ready = computed(() => Boolean(ballot.value) && !loading.value)

  function take(next: Ballot | null): void {
    ballot.value = next
    hold(next)
    prefetch(next)
  }

  async function open(resume = false): Promise<void> {
    if (resume) {
      const waiting = held(category.value)
      if (waiting) {
        ballot.value = waiting
        prefetch(waiting)
        return
      }
    }

    loading.value = true
    error.value = undefined
    throttled.value = false
    spent.value = null

    try {
      take(await api.rating.ballot(category.value))
    } catch (thrown) {
      take(null)
      note(thrown)
    } finally {
      loading.value = false
    }
  }

  async function vote(outcome: RatingOutcome): Promise<void> {
    const pending = ballot.value
    if (!pending || sending.value) return

    if (Date.now() - lastVote < pending.pace) return
    lastVote = Date.now()

    sending.value = true
    error.value = undefined
    cast.value += 1

    try {
      const answer = await api.rating.vote(pending.ballot, outcome, category.value)
      take(answer.next)
      if (!answer.next) await open()
    } catch (thrown) {
      cast.value -= 1
      note(thrown)
      if (spent.value) take(null)
      else if (thrown instanceof ApiError && thrown.status === 400) await open()
    } finally {
      sending.value = false
    }
  }

  async function reset(): Promise<void> {
    cast.value = 0
    lastVote = 0
    take(null)
    await open()
  }

  function note(thrown: unknown): void {
    if (thrown instanceof ApiError) {
      throttled.value = thrown.rateLimited
      spent.value = thrown.budget ?? null
      error.value = thrown.message
      return
    }
    spent.value = null
    error.value = thrown instanceof Error ? thrown.message : 'Something went wrong.'
  }

  return { ballot, loading, sending, error, throttled, spent, cast, ready, open, vote, reset }
}

const dismissed = ref(loadDismissed())

function loadDismissed(): boolean {
  try {
    return localStorage.getItem(CARD_KEY) === 'dismissed'
  } catch {
    return false
  }
}

export function hydrateRatingCard(): void {
  dismissed.value = loadDismissed()
}

export function useRatingCard() {
  function persist(): void {
    try {
      if (dismissed.value) localStorage.setItem(CARD_KEY, 'dismissed')
      else localStorage.removeItem(CARD_KEY)
    } catch {}
  }

  return {
    dismissed,
    dismiss: () => {
      dismissed.value = true
      persist()
    },
    reset: () => {
      dismissed.value = false
      persist()
    },
  }
}
