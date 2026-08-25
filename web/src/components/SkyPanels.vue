<script lang="ts" setup>
import { computed, onUnmounted, ref } from 'vue'
import KpGauge from './KpGauge.vue'
import MoonDial from './MoonDial.vue'
import type { Launch, SkyEventKind } from '@/api/types'
import { useSky } from '@/composables/useSky'
import { BAND_NAMES, BANDS, inForce, kpReading, levelName, NOTICE_LABELS } from '@/utils/weather'
import { RouterLink } from 'vue-router'

const { sky, failed, visiblePlanets } = useSky()

const DAY_MS = 86_400_000

const DATE = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' })
const TIME = new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit' })

const ICONS: Record<SkyEventKind, string> = {
  moon: 'pi pi-circle-fill',
  season: 'pi pi-sun',
  shower: 'pi pi-sparkles',
  eclipse: 'pi pi-circle',
  planet: 'pi pi-globe',
}

const FIRM = new Set(['SEC', 'MIN', 'HR', 'Second', 'Minute', 'Hour'])
const TICK_MS = 30_000

const clockNow = ref(Date.now())
const ticking = setInterval(() => (clockNow.value = Date.now()), TICK_MS)
onUnmounted(() => clearInterval(ticking))

function launchDetail(launch: Launch): string {
  const parts = passed(launch.net)
    ? [launch.status, launch.provider]
    : [launch.provider, launch.orbit]
  return parts.filter((part) => part && part !== 'Unknown' && part !== 'TBD').join(' · ')
}

function when(iso: string): string {
  const at = new Date(iso)
  return Number.isNaN(at.getTime()) ? iso : DATE.format(at)
}

function clock(iso: string): string {
  const at = new Date(iso)
  return Number.isNaN(at.getTime()) ? '' : TIME.format(at)
}

function passed(iso: string): boolean {
  const at = new Date(iso).getTime()
  return !Number.isNaN(at) && at < clockNow.value
}

function countdown(iso: string): string {
  const at = new Date(iso).getTime()
  if (Number.isNaN(at)) return ''

  const ms = at - clockNow.value
  const away = Math.abs(ms)
  const ago = ms < 0

  if (away < 60_000) return 'right now'
  if (away < 3_600_000) return step(Math.round(away / 60_000), 'min', ago)
  if (away < DAY_MS) return step(Math.round(away / 3_600_000), 'h', ago)

  const days = Math.round(away / DAY_MS)
  if (days === 1) return ago ? 'yesterday' : 'tomorrow'
  if (days < 45) return step(days, 'days', ago)

  const months = Math.round(days / 30.44)
  return step(months, months === 1 ? 'month' : 'months', ago)
}

function step(count: number, unit: string, ago: boolean): string {
  return ago ? `${count} ${unit} ago` : `in ${count} ${unit}`
}

const moon = computed(() => sky.value?.moon ?? null)

const NAMED_QUARTERS: Record<string, string> = { full: 'Next full', new: 'Next new' }

const nextQuarters = computed(() =>
  (moon.value?.next_quarters ?? [])
    .filter((quarter) => quarter.quarter in NAMED_QUARTERS)
    .map((quarter) => ({ ...quarter, name: NAMED_QUARTERS[quarter.quarter] as string }))
    .sort((one, other) => one.at.localeCompare(other.at)),
)

function thousands(km: number): string {
  return Math.round(km).toLocaleString()
}

const orbit = computed(() => {
  const now = moon.value
  if (!now) return 0

  const span = now.apogee_km - now.perigee_km
  if (span <= 0) return 0
  return Math.min(100, Math.max(0, ((now.distance_km - now.perigee_km) / span) * 100))
})

const report = computed(() => sky.value?.weather ?? null)

const activity = computed(() => (report.value ? kpReading(report.value.kp) : null))

const raised = computed(() => {
  const alert = report.value?.alert
  return alert && inForce(alert) ? alert : null
})

const stormy = computed(() => (report.value?.kp ?? 0) >= 5 || !!raised.value)

const levels = computed(() => {
  const stored = report.value?.scales?.levels ?? []
  return BANDS.map((band) => stored.find((level) => level.band === band)).filter(
    (level) => level !== undefined,
  )
})

const observed = computed(() => {
  const at = report.value?.observed_at
  if (!at) return ''
  return Number.isNaN(new Date(at).getTime()) ? '' : clock(at)
})

function magnitude(value: number): string {
  return `${value < 0 ? '−' : '+'}${Math.abs(value).toFixed(1)}`
}
</script>

<template>
  <template v-if="sky && moon">
    <div class="panels">
      <section class="card panel moon-panel">
        <h2 class="muted">The moon today</h2>

        <div class="row moon-row">
          <MoonDial :illumination="moon.illumination" :label="moon.label" :waxing="moon.waxing" />

          <div class="moon-facts">
            <p class="phase">{{ moon.label }}</p>
            <p class="muted lit">
              {{ Math.round(moon.illumination * 100) }}% lit,
              {{ moon.age_days < 1 ? 'less than a day' : `${Math.round(moon.age_days)} days` }} old
            </p>
            <p class="muted lit">
              {{ thousands(moon.distance_km) }} km away,
              <span class="drift">
                <i
                  :class="['pi', moon.closing ? 'pi-arrow-down-left' : 'pi-arrow-up-right']"
                  aria-hidden="true"
                />
                {{ moon.closing ? 'coming closer' : 'moving away' }}
              </span>
            </p>
          </div>
        </div>

        <div class="gauge orbit">
          <div class="track">
            <div :style="{ width: `${orbit}%` }" class="fill" />
            <span :style="{ left: `${orbit}%` }" class="pin" />
          </div>
          <p class="muted ends">
            <span>
              <strong>{{ thousands(moon.perigee_km) }} km</strong>
              at its closest
            </span>
            <span class="far">
              <strong>{{ thousands(moon.apogee_km) }} km</strong>
              at its farthest
            </span>
          </p>
        </div>

        <dl class="facts">
          <div v-for="quarter in nextQuarters" :key="quarter.quarter">
            <dt class="muted">{{ quarter.name }}</dt>
            <dd>
              {{ when(quarter.at) }} <span class="muted">{{ countdown(quarter.at) }}</span>
            </dd>
          </div>
        </dl>
      </section>

      <section class="card panel">
        <h2 class="muted">Visible planets</h2>

        <ul v-if="visiblePlanets.length" class="planets">
          <li
            v-for="planet in visiblePlanets"
            :key="planet.planet"
            :title="`${Math.round(planet.elongation)}° from the sun`"
          >
            <span class="planet-name">{{ planet.name }}</span>
            <span aria-hidden="true" class="leader" />
            <span class="muted where">{{ planet.visibility_label }}</span>
            <span class="mag">{{ magnitude(planet.magnitude) }}</span>
          </li>
        </ul>

        <p v-else class="muted note">All five currently appear too close to the sun.</p>

        <p class="muted note foot">
          Worked out from where each planet stands relative to the sun. Whether one of them clears
          your own horizon, and how high it gets, depends on your location.
        </p>
      </section>

      <section v-if="report" :class="{ stormy }" class="card panel">
        <h2 class="muted">Space weather</h2>

        <KpGauge :kp="report.kp" :stamp="observed" />

        <ul v-if="levels.length" class="bands">
          <li v-for="level in levels" :key="level.band" :class="{ up: (level.scale ?? 0) > 0 }">
            <span class="mark">{{ levelName(level) }}</span>
            <span class="muted band-name">{{ BAND_NAMES[level.band] }}</span>
          </li>
        </ul>

        <p v-if="raised" class="raised">
          <i aria-hidden="true" class="pi pi-exclamation-triangle" />
          <span>
            <strong>{{ NOTICE_LABELS[raised.notice] }}:</strong>
            {{ raised.headline }}
          </span>
        </p>
        <p v-else class="muted note">{{ activity?.note }}</p>

        <RouterLink class="more" to="/space-weather">
          Space weather in detail
          <i aria-hidden="true" class="pi pi-angle-right" />
        </RouterLink>
      </section>
    </div>

    <div class="columns">
      <section v-if="sky.events.length" class="card list">
        <h2 class="muted">Events in the sky</h2>

        <ol class="events">
          <li
            v-for="event in sky.events"
            :key="`${event.kind}-${event.at}`"
            :class="[event.kind, { gone: passed(event.at) }]"
          >
            <i :class="ICONS[event.kind]" aria-hidden="true" />
            <time :datetime="event.at" class="at">
              <span class="day">{{ when(event.at) }}</span>
              <span class="muted hour">{{ clock(event.at) }}</span>
            </time>
            <span class="what">
              <span class="title">{{ event.title }}</span>
              <span v-if="event.detail" class="muted detail">{{ event.detail }}</span>
            </span>
            <span class="muted away">{{ countdown(event.at) }}</span>
          </li>
        </ol>

        <p class="muted note">
          Calculated based on predictive models. If you notice any inaccuracies please
          <RouterLink to="/contact">contact me</RouterLink>
          .
        </p>
      </section>

      <section v-if="sky.launches.length" class="card list">
        <h2 class="muted">Rocket launches</h2>

        <ol class="events">
          <li
            v-for="launch in sky.launches"
            :key="launch.id"
            :class="{ gone: passed(launch.net) }"
            class="launch"
          >
            <i :class="passed(launch.net) ? 'pi pi-check' : 'pi pi-send'" aria-hidden="true" />
            <time :datetime="launch.net" class="at">
              <span class="day">{{ when(launch.net) }}</span>
              <span class="muted hour">{{
                launch.precision && FIRM.has(launch.precision) ? clock(launch.net) : 'time to come'
              }}</span>
            </time>
            <span class="what">
              <a
                v-if="launch.info_url"
                :href="launch.info_url"
                class="title link"
                data-ours
                rel="noopener"
                target="_blank"
              >
                {{ launch.name }}
                <i aria-hidden="true" class="pi pi-external-link" />
              </a>
              <span v-else class="title">{{ launch.name }}</span>

              <span v-if="launchDetail(launch)" class="muted detail">{{
                launchDetail(launch)
              }}</span>
            </span>
            <span class="muted away">{{ countdown(launch.net) }}</span>
          </li>
        </ol>

        <p class="muted note">
          Data from
          <a data-ours href="https://thespacedevs.com" rel="noopener" target="_blank"
            >The Space Devs</a
          >. This information could potentially be outdated since launch windows can reschedule
          without warning.
        </p>
      </section>
    </div>
  </template>

  <div v-else-if="!failed" class="panels">
    <div class="card panel">
      <Skeleton height="8rem" />
    </div>
    <div class="card panel">
      <Skeleton height="8rem" />
    </div>
  </div>
</template>

<style scoped>
.panels {
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 17rem), 1fr));
}

.panel {
  padding: 1.1rem 1.2rem 1.25rem;
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
}

h2 {
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.07em;
  font-weight: 600;
}

.moon-row {
  gap: 1rem;
  flex-wrap: nowrap;
}

.moon-facts {
  min-width: 0;
}

.phase {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 620;
  letter-spacing: -0.02em;
  line-height: 1.25;
  text-wrap: balance;
}

.lit {
  margin: 0;
  font-size: 0.85rem;
  font-variant-numeric: tabular-nums;
}

.drift {
  white-space: nowrap;
}

.drift i {
  font-size: 0.75em;
  margin-right: 0.1rem;
  color: var(--accent);
}

.facts {
  display: flex;
  gap: 1.5rem;
  margin: 0;
  flex-wrap: wrap;
}

.facts dt {
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.facts dd {
  margin: 0;
  font-size: 0.95rem;
}

.facts dd span {
  font-size: 0.8rem;
}

.note {
  margin: 0;
  font-size: 0.8rem;
  text-wrap: pretty;
}

.planets {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.planets li {
  display: flex;
  align-items: baseline;
  gap: 0.4rem;
  font-size: 0.92rem;
}

.planet-name {
  font-weight: 550;
}

.leader {
  flex: 1;
  border-bottom: 1px dotted color-mix(in srgb, var(--text) 25%, transparent);
  min-width: 1rem;
  transform: translateY(-0.2em);
}

.where {
  font-size: 0.82rem;
  white-space: nowrap;
}

.foot {
  margin-top: auto;
  font-size: 0.72rem;
}

.mag {
  font-variant-numeric: tabular-nums;
  font-size: 0.88rem;
  min-width: 3ch;
  text-align: right;
}

.stormy {
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
}

.gauge {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.track {
  position: relative;
  height: 0.5rem;
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

.orbit .track {
  overflow: visible;
}

.orbit .fill {
  border-radius: 999px 0 0 999px;
}

.pin {
  position: absolute;
  top: 50%;
  width: 0.5rem;
  height: 0.5rem;
  margin-left: -0.25rem;
  transform: translateY(-50%);
  border-radius: 999px;
  background: var(--bg-elevated);
  border: 2px solid var(--accent);
}

.ends {
  display: flex;
  justify-content: space-between;
  gap: 0.75rem;
  margin: 0;
  font-size: 0.72rem;
  line-height: 1.4;
  font-variant-numeric: tabular-nums;
}

.ends span {
  display: flex;
  flex-direction: column;
}

.ends strong {
  font-weight: 600;
}

.far {
  text-align: right;
}

.bands {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  font-size: 0.82rem;
}

.bands li {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
}

.mark {
  min-width: 2.6rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 0 0.45rem;
  text-align: center;
  font-size: 0.72rem;
  font-weight: 600;
}

.bands li.up .mark {
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  background: color-mix(in srgb, var(--accent) 16%, transparent);
  color: var(--accent);
}

.band-name {
  font-size: 0.8rem;
}

.raised {
  display: flex;
  align-items: baseline;
  gap: 0.4rem;
  margin: 0;
  font-size: 0.85rem;
  text-wrap: pretty;
}

.raised i {
  color: var(--accent);
  font-size: 0.9em;
}

.more {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
  margin-top: auto;
  font-size: 0.85rem;
  text-decoration: none;
  width: fit-content;
}

.more:hover {
  text-decoration: underline;
}

.columns {
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.list {
  padding: 1.1rem 1.2rem 1.25rem;
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
  container-type: inline-size;
}

@media (max-width: 62rem) {
  .columns {
    grid-template-columns: minmax(0, 1fr);
  }
}

.events {
  list-style: none;
  margin: 0;
  padding: 0 0.35rem 0 0;
  display: flex;
  flex-direction: column;
  max-height: 28rem;
  overflow-y: auto;
  overscroll-behavior: contain;
}

.events li {
  display: grid;
  grid-template-columns: 1.4rem 5.5rem minmax(0, 1fr) auto;
  align-items: baseline;
  gap: 0.7rem;
  padding: 0.5rem 0;
  border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
}

.events li:first-child {
  border-top: none;
}

.events li.gone {
  opacity: 0.62;
}

.events li.gone:hover,
.events li.gone:focus-within {
  opacity: 1;
}

.events li.gone .day {
  font-weight: 450;
}

.events li.gone .away {
  font-style: italic;
}

.events i {
  color: var(--text-muted);
  font-size: 0.8rem;
  justify-self: center;
}

.events li.eclipse i,
.events li.shower i {
  color: var(--accent);
}

.at {
  display: flex;
  flex-direction: column;
  line-height: 1.3;
}

.day {
  font-size: 0.88rem;
  font-weight: 550;
  white-space: nowrap;
}

.hour {
  font-size: 0.75rem;
  font-variant-numeric: tabular-nums;
}

.what {
  display: flex;
  flex-direction: column;
  min-width: 0;
  line-height: 1.35;
}

.title {
  font-size: 0.94rem;
  text-wrap: pretty;
}

a.title {
  color: inherit;
  text-decoration: none;
  width: fit-content;
}

a.title:hover,
a.title:focus-visible {
  color: var(--accent);
}

a.title i {
  font-size: 0.62em;
  opacity: 0.55;
  vertical-align: 0.15em;
  margin-left: 0.15rem;
}

a.title:hover i {
  opacity: 1;
}

.detail {
  font-size: 0.8rem;
  text-wrap: pretty;
}

.away {
  font-size: 0.8rem;
  white-space: nowrap;
}

@container (max-width: 30rem) {
  .events li {
    grid-template-columns: 1.2rem minmax(0, 1fr) auto;
    row-gap: 0.15rem;
    column-gap: 0.6rem;
    align-items: start;
  }

  .events i {
    grid-column: 1;
    grid-row: 1 / span 2;
    padding-top: 0.2rem;
  }

  .at {
    grid-column: 2;
    grid-row: 1;
    flex-direction: row;
    align-items: baseline;
    gap: 0.4rem;
  }

  .away {
    grid-column: 3;
    grid-row: 1;
  }

  .what {
    grid-column: 2 / -1;
    grid-row: 2;
  }
}
</style>
