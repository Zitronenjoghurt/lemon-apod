<script lang="ts" setup>
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import ApodBirthday from '@/components/ApodBirthday.vue'
import ApodCredit from '@/components/ApodCredit.vue'
import RatingCard from '@/components/rating/RatingCard.vue'
import ReadProgress from '@/components/ReadProgress.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import SkyPanels from '@/components/SkyPanels.vue'
import WelcomeNote from '@/components/WelcomeNote.vue'
import { api } from '@/api/client'
import { type ApodEntry, type ApodSummary, isVideo } from '@/api/types'
import { useCoverage } from '@/composables/useCoverage'
import { useRead } from '@/composables/useRead'
import { useStatus } from '@/composables/useStatus'
import { apodPageUrl } from '@/utils/apodLinks'
import { apodAgeParts, formatDate } from '@/utils/date'

const TICK_MS = 1_000
const POLL_MS = 30_000

const { latest, entries, publish, pause, pauseRunning, refresh } = useStatus()
const { countIn, isRead } = useRead()
const coverage = useCoverage()

const now = ref(Date.now())
const error = ref<string>()
const loading = ref(false)

const localToday = computed(() => {
  const at = new Date(now.value)
  const month = String(at.getMonth() + 1).padStart(2, '0')
  const day = String(at.getDate()).padStart(2, '0')
  return `${at.getFullYear()}-${month}-${day}`
})

const standing = computed<'ahead' | 'level' | 'behind' | 'unknown'>(() => {
  if (!publish.value) return 'unknown'
  if (localToday.value > publish.value.today) return 'ahead'
  if (localToday.value < publish.value.today) return 'behind'
  return 'level'
})

const localEntry = ref<ApodSummary | null>(null)

async function loadLocalDay() {
  if (standing.value !== 'behind') {
    localEntry.value = null
    return
  }

  const date = localToday.value
  try {
    const page = await api.entries({ from: date, to: date, limit: 1 })
    localEntry.value = page.items[0] ?? null
  } catch {
    localEntry.value = null
  }
}

const featured = computed(() => (standing.value === 'behind' ? localEntry.value : latest.value))

const alreadyUp = computed(() => {
  if (standing.value !== 'behind' || !latest.value) return null
  return featured.value && latest.value.date > featured.value.date ? latest.value : null
})

const caughtUp = computed(() =>
  Boolean(publish.value && latest.value && latest.value.date >= publish.value.today),
)

const headline = computed(() => {
  if (pauseRunning.value) return 'The last picture published'

  switch (standing.value) {
    case 'behind':
      return featured.value ? "Today's picture" : "Nothing archived for 'your today' yet"
    case 'level':
      return caughtUp.value ? "Today's picture" : "Today's picture is not up yet"
    case 'ahead':
      return 'The most recent picture'
    default:
      return 'The most recent picture'
  }
})

const millisToPublish = computed(() => {
  if (!publish.value) return null
  const at = Date.parse(publish.value.next_at)
  return Number.isNaN(at) ? null : at - now.value
})

const eta = computed(() => {
  const left = millisToPublish.value
  if (left === null) return null
  if (left <= 0) return { parts: [], soon: 'any moment now' }

  const total = Math.floor(left / 1_000)
  const hours = Math.floor(total / 3_600)
  const minutes = Math.floor((total % 3_600) / 60)
  const seconds = total % 60

  const parts: { value: string; unit: string }[] = []
  if (hours) parts.push({ value: String(hours), unit: hours === 1 ? 'hour' : 'hours' })
  if (hours || minutes) {
    parts.push({ value: hours ? String(minutes).padStart(2, '0') : String(minutes), unit: 'min' })
  }
  parts.push({
    value: parts.length ? String(seconds).padStart(2, '0') : String(seconds),
    unit: 'sec',
  })

  return { parts }
})

const localPublishTime = computed(() => {
  if (!publish.value) return null
  const at = new Date(Date.parse(publish.value.next_at))
  return Number.isNaN(at.getTime())
    ? null
    : at.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })
})

const clockLabel = computed(() => {
  if (!publish.value) return ''
  const { hour, minute, abbreviation } = publish.value
  if (hour === 0 && minute === 0) return `midnight ${abbreviation}`
  return `${String(hour).padStart(2, '0')}:${String(minute).padStart(2, '0')} ${abbreviation}`
})

const age = computed(() => apodAgeParts(publish.value?.today ?? localToday.value))

const pauseSince = computed(() => (pause.value ? formatDate(pause.value.start) : ''))
const pauseUntil = computed(() => (pause.value?.end ? formatDate(pause.value.end) : ''))

const archiveRead = computed(() => countIn())
const archiveTotal = computed(() => coverage.total.value || entries.value)

const featuredFull = ref<ApodEntry | null>(null)

const credits = computed(() =>
  (featuredFull.value?.credits ?? []).map((credit) => `${credit.role}: ${credit.text}`),
)

async function loadCredits() {
  const date = featured.value?.date
  featuredFull.value = null
  if (!date) return

  try {
    const full = await api.entry(date)
    if (featured.value?.date === date) featuredFull.value = full
  } catch {
    featuredFull.value = null
  }
}

async function reload() {
  loading.value = true
  error.value = undefined
  try {
    await refresh()
    if (!latest.value) error.value = 'Could not reach the archive.'
  } finally {
    loading.value = false
  }
}

let timer: ReturnType<typeof setInterval> | undefined
let poller: ReturnType<typeof setInterval> | undefined

onMounted(() => {
  timer = setInterval(() => (now.value = Date.now()), TICK_MS)
  poller = setInterval(() => {
    if ((millisToPublish.value ?? 1) <= 0) void refresh()
  }, POLL_MS)
})

onUnmounted(() => {
  clearInterval(timer)
  clearInterval(poller)
})

watch([standing, localToday], loadLocalDay, { immediate: true })

watch(() => featured.value?.date, loadCredits, { immediate: true })
</script>

<template>
  <div class="stack status">
    <WelcomeNote />

    <section v-if="pauseRunning && pause" class="paused">
      <AppIcon name="exclamation-triangle" />
      <div class="body">
        <h2>APOD is on hiatus!</h2>
        <p>
          There have not been any new posts since {{ pauseSince
          }}<template v-if="pauseUntil"> but it is expected back on {{ pauseUntil }}</template
          >.
        </p>
        <p v-if="pause.reason" class="reason">Reason: {{ pause.reason }}</p>
      </div>
    </section>

    <RetryNotice v-if="error" :busy="loading" :message="error" @retry="reload" />

    <div v-else-if="!publish || (!featured && standing !== 'behind')" class="stack">
      <Skeleton height="2rem" width="14rem" />
      <Skeleton height="12rem" width="100%" />
    </div>

    <template v-else>
      <section class="card today">
        <RouterLink v-if="featured" :to="`/${featured.date}`" class="thumb">
          <img
            v-if="featured.media.thumb_url"
            :alt="featured.title"
            :src="featured.media.thumb_url"
            decoding="async"
            height="300"
            width="480"
          />
          <div v-else class="fallback">
            <AppIcon name="image" />
          </div>
          <span v-if="isVideo(featured.media.kind)" aria-label="Video" class="badge">
            <AppIcon name="play" />
          </span>
        </RouterLink>

        <div class="body">
          <p class="muted kicker">
            <span :class="{ live: caughtUp }" aria-hidden="true" class="dot" />
            {{ headline }}
          </p>

          <template v-if="featured">
            <h2 class="title">
              <RouterLink :to="`/${featured.date}`">{{ featured.title }}</RouterLink>
            </h2>
            <p class="muted date">
              <time :datetime="featured.date">{{ formatDate(featured.date) }}</time>
              <ApodBirthday :date="featured.date" />
              <span v-if="isRead(featured.date)" class="tag-read">
                <AppIcon name="check" /> Read
              </span>
            </p>

            <div class="attribution">
              <ApodCredit
                :source="featuredFull ? apodPageUrl(featuredFull) : undefined"
                lead="NASA's"
                variant="banner"
              />
              <p v-if="credits.length" class="muted credit">
                <span v-for="line in credits" :key="line">{{ line }}</span>
              </p>
            </div>

            <div class="row actions">
              <RouterLink v-slot="{ navigate }" :to="`/${featured.date}`" custom>
                <Button label="Read it" @click="navigate">
                  <template #icon><AppIcon name="book" /></template>
                </Button>
              </RouterLink>
              <RouterLink v-slot="{ navigate }" custom to="/random">
                <Button label="Read a random entry" outlined severity="secondary" @click="navigate">
                  <template #icon><AppIcon name="random" /></template>
                </Button>
              </RouterLink>
            </div>
          </template>

          <p v-else class="muted empty">
            <template v-if="pauseRunning">
              APOD published nothing for {{ formatDate(localToday) }}.
            </template>
            <template v-else>
              The archiver has not archived the entry for {{ formatDate(localToday) }} yet.
            </template>
          </p>
        </div>
      </section>

      <section v-if="alreadyUp && !pauseRunning" class="card ahead">
        <AppIcon name="forward" />
        <p>
          Tomorrow's picture has already been released. It is past {{ clockLabel }}, where APOD
          publishes, even though it is still {{ formatDate(localToday) }} where you are.
        </p>
        <RouterLink v-slot="{ navigate }" :to="`/${alreadyUp.date}`" custom>
          <Button
            icon-pos="right"
            label="See it"
            outlined
            severity="secondary"
            size="small"
            @click="navigate"
          >
            <template #icon><AppIcon name="arrow-right" /></template>
          </Button>
        </RouterLink>
      </section>

      <div class="panels">
        <section v-if="!pauseRunning" class="card panel">
          <h2 class="muted">Next picture most likely in</h2>

          <p v-if="eta" class="countdown">
            <template v-if="eta.soon">
              <span class="soon">{{ eta.soon }}</span>
            </template>
            <template v-else>
              <span v-for="part in eta.parts" :key="part.unit" class="part">
                <span class="figure">{{ part.value }}</span>
                <span class="unit muted">{{ part.unit }}</span>
              </span>
            </template>
          </p>

          <dl class="facts">
            <div>
              <dt class="muted">Published at</dt>
              <dd>{{ clockLabel }}</dd>
            </div>
            <div v-if="localPublishTime">
              <dt class="muted">Your time</dt>
              <dd>{{ localPublishTime }}</dd>
            </div>
          </dl>

          <p v-if="standing === 'level' && !caughtUp && !pauseRunning" class="muted note">
            The entry for {{ formatDate(publish.today) }} has not been archived yet.
          </p>
        </section>

        <section class="card panel">
          <h2 class="muted">Your reading progress</h2>

          <p class="countdown">
            <span class="part">
              <span class="figure">{{ archiveRead.toLocaleString() }}</span>
              <span class="unit muted">of {{ archiveTotal.toLocaleString() }}</span>
            </span>
          </p>

          <ReadProgress :read="archiveRead" :total="archiveTotal" bare label="the archive" />

          <p class="muted note">
            Reading progress only lives in your browser. You can back it up like other site data in
            the settings.
          </p>
        </section>

        <section class="card panel">
          <h2 class="muted">Astronomy Picture of the Day is</h2>

          <p class="countdown">
            <span class="part">
              <span class="figure">{{ age.years }}</span>
              <span class="unit muted">years</span>
            </span>
            <span class="part">
              <span class="figure">{{ age.days }}</span>
              <span class="unit muted">days old</span>
            </span>
          </p>

          <p class="muted note">
            APOD started on
            <RouterLink to="/1995-06-16">16 June 1995</RouterLink>
            and has run almost every day since then.
          </p>
        </section>
      </div>

      <RatingCard />

      <SkyPanels />
    </template>
  </div>
</template>

<style scoped>
.status {
  gap: var(--space-5);
}

.paused {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  line-height: 1.45;
  padding: var(--space-4) var(--space-5);
  border: 1px solid hsl(var(--tone-raised) / 0.55);
  border-left-width: 4px;
  border-radius: var(--radius);
  background: hsl(var(--tone-raised) / 0.12);
}

.paused > .icon {
  color: hsl(var(--tone-raised));
  font-size: var(--text-md);
  line-height: inherit;
}

.paused .body {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  flex: 1;
}

.paused h2 {
  margin: 0;
  font-size: var(--text-md);
  line-height: inherit;
  text-wrap: balance;
}

.paused p {
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.paused .reason {
  color: var(--text-muted);
}

h1 {
  font-size: var(--text-xl);
}

.panel h2 {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.07em;
  font-weight: 600;
}

.today {
  display: grid;
  grid-template-columns: minmax(0, 22rem) minmax(0, 1fr);
  overflow: hidden;
}

.thumb {
  position: relative;
  display: block;
  background: color-mix(in srgb, var(--text) 6%, transparent);
  min-height: 14rem;
}

.thumb img,
.thumb .fallback {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.fallback {
  display: grid;
  place-items: center;
  color: var(--text-muted);
  font-size: 1.8rem;
}

.badge {
  position: absolute;
  right: 0.6rem;
  bottom: 0.6rem;
  display: grid;
  place-items: center;
  width: 1.9rem;
  height: 1.9rem;
  border-radius: 50%;
  background: rgb(0 0 0 / 0.55);
  color: #fff;
  font-size: var(--text-xs);
}

.body {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4) var(--space-5);
  min-width: 0;
}

.kicker {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.dot {
  width: 0.45rem;
  height: 0.45rem;
  border-radius: 50%;
  background: color-mix(in srgb, var(--text) 35%, transparent);
  flex: none;
}

.dot.live {
  background: var(--accent);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
}

.title {
  font-size: var(--text-title);
  text-wrap: balance;
}

.title a {
  color: inherit;
  text-decoration: none;
}

.title a:hover {
  color: var(--accent);
}

.date {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-sm);
}

.tag-read {
  font-size: var(--text-xs);
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  padding: 0 var(--space-2);
}

.tag-read .icon {
  font-size: 0.7em;
}

.attribution {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin: var(--space-1) 0 var(--space-0);
  padding-left: var(--space-3);
  border-left: 2px solid color-mix(in srgb, var(--accent) 55%, transparent);
}

.credit {
  display: flex;
  flex-direction: column;
  gap: var(--space-0);
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.actions {
  gap: var(--space-2);
  margin-top: var(--space-1);
}

.empty {
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.ahead {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
  flex-wrap: wrap;
}

.ahead .icon {
  color: var(--accent);
  flex: none;
}

.ahead p {
  margin: 0;
  flex: 1 1 18rem;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.panels {
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 17rem), 1fr));
}

.panel {
  padding: var(--space-4) var(--space-5) var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.countdown {
  display: flex;
  align-items: baseline;
  gap: var(--space-3);
  margin: 0;
  flex-wrap: wrap;
}

.part {
  display: flex;
  align-items: baseline;
  gap: var(--space-1);
}

.figure {
  font-size: 2.1rem;
  font-weight: 650;
  line-height: 1;
  letter-spacing: -0.03em;
  font-variant-numeric: tabular-nums;
}

.unit {
  font-size: var(--text-sm);
}

.soon {
  font-size: var(--text-title);
  font-weight: 600;
}

.facts {
  display: flex;
  gap: var(--space-6);
  margin: 0;
  flex-wrap: wrap;
}

.facts dt {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.facts dd {
  margin: 0;
  font-size: var(--text-md);
  font-variant-numeric: tabular-nums;
}

.note {
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
  margin-top: auto;
}

@media (max-width: 44rem) {
  .today {
    grid-template-columns: minmax(0, 1fr);
  }

  .thumb {
    aspect-ratio: 16 / 9;
  }

  .title {
    font-size: var(--text-lg);
  }
}
</style>
