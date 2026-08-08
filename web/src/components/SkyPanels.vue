<script lang="ts" setup>
import { computed } from 'vue'
import MoonDial from './MoonDial.vue'
import type { Launch, SkyEventKind } from '@/api/types'
import { useSky } from '@/composables/useSky'
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

function launchDetail(launch: Launch): string {
  return [launch.provider, launch.orbit].filter((part) => part && part !== 'Unknown').join(' · ')
}

function when(iso: string): string {
  const at = new Date(iso)
  return Number.isNaN(at.getTime()) ? iso : DATE.format(at)
}

function clock(iso: string): string {
  const at = new Date(iso)
  return Number.isNaN(at.getTime()) ? '' : TIME.format(at)
}

function countdown(iso: string): string {
  const at = new Date(iso).getTime()
  if (Number.isNaN(at)) return ''

  const ms = at - Date.now()
  if (ms < 0) return 'now'
  if (ms < 3_600_000) return `in ${Math.max(1, Math.round(ms / 60_000))} min`
  if (ms < DAY_MS) return `in ${Math.round(ms / 3_600_000)} h`

  const days = Math.round(ms / DAY_MS)
  if (days === 1) return 'tomorrow'
  if (days < 45) return `in ${days} days`

  const months = Math.round(days / 30.44)
  return `in ${months} month${months === 1 ? '' : 's'}`
}

const moon = computed(() => sky.value?.moon ?? null)

const nextNew = computed(() =>
  moon.value?.next_quarters.find((quarter) => quarter.quarter === 'new'),
)
const nextFull = computed(() =>
  moon.value?.next_quarters.find((quarter) => quarter.quarter === 'full'),
)

const distance = computed(() =>
  moon.value ? Math.round(moon.value.distance_km).toLocaleString() : '',
)

const weather = computed(() => sky.value?.space_weather ?? null)

const SWPC_URL = 'https://www.swpc.noaa.gov/products/planetary-k-index'

const KP_SCALE: { at: number; label: string; note: string }[] = [
  {
    at: 9,
    label: 'Extreme storm (G5)',
    note: 'The aurora may be visible as far south as 40 degrees latitude.',
  },
  {
    at: 8,
    label: 'Severe storm (G4)',
    note: 'The aurora may be visible as far south as 45 degrees latitude.',
  },
  {
    at: 7,
    label: 'Strong storm (G3)',
    note: 'The aurora may be visible as far south as 50 degrees latitude.',
  },
  {
    at: 6,
    label: 'Moderate storm (G2)',
    note: 'The aurora may be visible as far south as 55 degrees latitude.',
  },
  {
    at: 5,
    label: 'Minor storm (G1)',
    note: 'The aurora may be visible as far south as 60 degrees latitude.',
  },
  {
    at: 4,
    label: 'Unsettled',
    note: 'Busier than usual, but not a storm. The aurora stays near the poles.',
  },
  { at: 0, label: 'Quiet', note: 'A normal day. The aurora stays near the poles.' },
]

const activity = computed(() => {
  const kp = weather.value?.kp
  if (kp === undefined) return null
  return KP_SCALE.find((step) => kp >= step.at) ?? KP_SCALE[KP_SCALE.length - 1]!
})

const stormy = computed(() => (weather.value?.kp ?? 0) >= 5)

const kpPercent = computed(() =>
  weather.value ? Math.min(100, Math.max(0, (weather.value.kp / 9) * 100)) : 0,
)

const observed = computed(() => {
  if (!weather.value) return ''
  const at = new Date(weather.value.observed_at)
  return Number.isNaN(at.getTime()) ? '' : clock(weather.value.observed_at)
})

function magnitude(value: number): string {
  return `${value < 0 ? '−' : '+'}${Math.abs(value).toFixed(1)}`
}
</script>

<template>
  <template v-if="sky && moon">
    <div class="panels">
      <section class="card panel moon-panel">
        <h2 class="muted">The moon tonight</h2>

        <div class="row moon-row">
          <MoonDial :illumination="moon.illumination" :label="moon.label" :waxing="moon.waxing" />

          <div class="moon-facts">
            <p class="phase">{{ moon.label }}</p>
            <p class="muted lit">
              {{ Math.round(moon.illumination * 100) }}% lit,
              {{ moon.age_days < 1 ? 'less than a day' : `${Math.round(moon.age_days)} days` }} old
            </p>
            <p class="muted lit">{{ distance }} km away</p>
          </div>
        </div>

        <dl class="facts">
          <div v-if="nextFull">
            <dt class="muted">Next full</dt>
            <dd>
              {{ when(nextFull.at) }} <span class="muted">{{ countdown(nextFull.at) }}</span>
            </dd>
          </div>
          <div v-if="nextNew">
            <dt class="muted">Next new</dt>
            <dd>
              {{ when(nextNew.at) }} <span class="muted">{{ countdown(nextNew.at) }}</span>
            </dd>
          </div>
        </dl>
      </section>

      <section class="card panel">
        <h2 class="muted">Planets tonight</h2>

        <ul v-if="visiblePlanets.length" class="planets">
          <li v-for="planet in visiblePlanets" :key="planet.planet">
            <span class="planet-name">{{ planet.name }}</span>
            <span aria-hidden="true" class="leader" />
            <span class="muted where">{{
              planet.visibility === 'evening' ? 'Evening' : 'Morning'
            }}</span>
            <span class="mag">{{ magnitude(planet.magnitude) }}</span>
          </li>
        </ul>

        <p v-else class="muted note">No planets seem to be visible tonight.</p>
      </section>

      <section v-if="weather && activity" :class="{ stormy }" class="card panel">
        <h2 class="muted">Space weather</h2>

        <div class="row kp-row">
          <p class="kp">
            <span class="figure">{{ weather.kp.toFixed(2) }}</span>
            <span class="unit muted">Kp</span>
          </p>
          <p class="verdict">{{ activity.label }}</p>
        </div>

        <div class="gauge">
          <div class="track">
            <div :style="{ width: `${kpPercent}%` }" class="fill" />
            <div class="threshold" />
          </div>
          <p class="muted caption">0 to 9. Storms start at 5.</p>
        </div>

        <p class="muted note">{{ activity.note }}</p>

        <p class="muted source">
          <a :href="SWPC_URL" data-ours rel="noopener" target="_blank"
            >NOAA Space Weather Prediction Center</a
          >
          ({{ observed }})
        </p>
      </section>
    </div>

    <div class="columns">
      <section v-if="sky.events.length" class="card list">
        <h2 class="muted">Events in the sky</h2>

        <ol class="events">
          <li v-for="event in sky.events" :key="`${event.kind}-${event.at}`" :class="event.kind">
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
          <li v-for="launch in sky.launches" :key="launch.id" class="launch">
            <i aria-hidden="true" class="pi pi-send" />
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

.kp-row {
  justify-content: space-between;
  gap: 0.75rem;
}

.kp {
  display: flex;
  align-items: baseline;
  gap: 0.35rem;
  margin: 0;
}

.verdict {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 600;
  text-align: right;
  text-wrap: balance;
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

.threshold {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 55.55%;
  width: 2px;
  background: color-mix(in srgb, var(--text) 45%, transparent);
}

.caption {
  margin: 0;
  font-size: 0.72rem;
}

.source {
  margin: 0;
  font-size: 0.75rem;
  text-wrap: pretty;
}

.columns {
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(2, minmax(0, 1fr));
  align-items: start;
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
  padding: 0;
  display: flex;
  flex-direction: column;
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
