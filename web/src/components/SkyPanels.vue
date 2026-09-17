<script lang="ts" setup>
import { computed, onUnmounted, ref } from 'vue'
import EclipseDial from './EclipseDial.vue'
import KpGauge from './KpGauge.vue'
import MagnitudeMark from './MagnitudeMark.vue'
import MoonDial from './MoonDial.vue'
import RangeTrack from './RangeTrack.vue'
import type { Launch } from '@/api/types'
import { useLaunches } from '@/composables/useLaunches'
import { usePreferences } from '@/composables/usePreferences'
import { useSky } from '@/composables/useSky'
import {
  clockOf,
  countdown as away,
  isImminent,
  hasOutcome,
  launchMark,
  statusTone,
  timeOf,
} from '@/utils/launches'
import {
  EVENT_ICONS,
  eventColor,
  eventDetail,
  planetColor,
  planetIcon,
  planetScale,
  VISIBILITY,
} from '@/utils/sky'
import { BAND_NAMES, BANDS, inForce, kpReading, levelName, NOTICE_LABELS } from '@/utils/weather'
import { RouterLink } from 'vue-router'

const { sky, failed, visiblePlanets } = useSky()
const { hemisphere } = usePreferences()
const { data: launchFeed } = useLaunches()

const LAUNCHES_BEHIND = 3
const LAUNCHES_AHEAD = 10

const launches = computed(() => {
  const ahead = launchFeed.value?.upcoming ?? []
  const gone = launchFeed.value?.flown ?? []

  const soon = [...ahead, ...gone]
    .filter(imminent)
    .sort((one, other) => Date.parse(one.net) - Date.parse(other.net))
  const behind = gone
    .filter((launch) => !imminent(launch))
    .slice(0, LAUNCHES_BEHIND)
    .reverse()
  const rest = ahead.filter((launch) => !imminent(launch)).slice(0, LAUNCHES_AHEAD)

  return [...soon, ...behind, ...rest]
})

function imminent(launch: Launch): boolean {
  return isImminent(launch, clockNow.value)
}

function lost(launch: Launch): boolean {
  return statusTone(launch.status) === 'bad'
}

function launchTone(launch: Launch): 'lost' | 'flown' | 'soon' | 'ahead' {
  if (lost(launch)) return 'lost'
  if (hasOutcome(launch.status)) return 'flown'
  return imminent(launch) ? 'soon' : 'ahead'
}

const DATE = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' })

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

function passed(iso: string): boolean {
  const at = new Date(iso).getTime()
  return !Number.isNaN(at) && at < clockNow.value
}

function countdown(iso: string): string {
  return away(iso, clockNow.value)
}

const EVENTS_SHOWN = 10

const events = computed(() => (sky.value?.events ?? []).slice(0, EVENTS_SHOWN))

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
  return timeOf(at)
})

function magnitude(value: number): string {
  return `${value < 0 ? '−' : '+'}${Math.abs(value).toFixed(1)}`
}
</script>

<template>
  <template v-if="sky && moon">
    <div class="panels">
      <section class="card panel moon-panel rise">
        <h2 class="muted">
          <AppIcon class="lead" name="moon" />
          <RouterLink class="open" to="/sky">
            The moon today
            <AppIcon name="chevron-right" />
          </RouterLink>
        </h2>

        <div class="row moon-row">
          <MoonDial
            :hemisphere="hemisphere"
            :illumination="moon.illumination"
            :label="moon.label"
            :waxing="moon.waxing"
          />

          <div class="moon-facts">
            <p class="phase">{{ moon.label }}</p>
            <p class="muted lit">
              {{ Math.round(moon.illumination * 100) }}% lit,
              {{ moon.age_days < 1 ? 'less than a day' : `${Math.round(moon.age_days)} days` }} old
            </p>
            <p class="muted lit">
              {{ thousands(moon.distance_km) }} km away,
              <span class="drift">
                <AppIcon :name="moon.closing ? 'arrow-down-left' : 'arrow-up-right'" />
                {{ moon.closing ? 'coming closer' : 'moving away' }}
              </span>
            </p>
          </div>
        </div>

        <RangeTrack
          :max="moon.apogee_km"
          :max-label="`${thousands(moon.apogee_km)} km`"
          max-note="at its farthest"
          :min="moon.perigee_km"
          :min-label="`${thousands(moon.perigee_km)} km`"
          min-note="at its closest"
          :value="moon.distance_km"
        />

        <dl class="facts">
          <div v-for="quarter in nextQuarters" :key="quarter.quarter">
            <dt class="muted">{{ quarter.name }}</dt>
            <dd>
              {{ when(quarter.at) }} <span class="muted">{{ countdown(quarter.at) }}</span>
            </dd>
          </div>
        </dl>
      </section>

      <section class="card panel rise">
        <h2 class="muted">
          <AppIcon class="lead" name="planets" />
          <RouterLink class="open" to="/sky">
            Visible planets
            <AppIcon name="chevron-right" />
          </RouterLink>
        </h2>

        <ul v-if="visiblePlanets.length" class="planets">
          <li
            v-for="planet in visiblePlanets"
            :key="planet.planet"
            :title="`${Math.round(planet.elongation)}° from the sun`"
          >
            <AppIcon
              :name="planetIcon(planet.planet)"
              :scale="planetScale(planet.planet)"
              :style="{ color: planetColor(planet.planet) }"
              class="planet-mark"
            />
            <span class="planet-name">{{ planet.name }}</span>
            <span aria-hidden="true" class="leader" />
            <span :class="['where', VISIBILITY[planet.visibility].tone]">
              <AppIcon :name="VISIBILITY[planet.visibility].icon" />
              {{ planet.visibility_label }}
            </span>
            <span :title="`Apparent magnitude ${magnitude(planet.magnitude)}`" class="mag">
              <MagnitudeMark :magnitude="planet.magnitude" />
              {{ magnitude(planet.magnitude) }}
            </span>
          </li>
        </ul>

        <p v-else class="muted note">All five currently appear too close to the sun.</p>
      </section>

      <section v-if="report" :class="{ stormy }" class="card panel rise">
        <h2 class="muted">
          <AppIcon class="lead" name="bolt" />
          <RouterLink class="open" to="/space-weather">
            Space weather
            <AppIcon name="chevron-right" />
          </RouterLink>
        </h2>

        <KpGauge :kp="report.kp" :stamp="observed" />

        <p v-if="levels.length" class="muted scope">Worst so far today</p>
        <ul v-if="levels.length" class="bands">
          <li v-for="level in levels" :key="level.band" :class="{ up: (level.scale ?? 0) > 0 }">
            <span class="mark">{{ levelName(level) }}</span>
            <span class="muted band-name">{{ BAND_NAMES[level.band] }}</span>
          </li>
        </ul>

        <p v-if="raised" class="raised">
          <AppIcon name="exclamation-triangle" />
          <span>
            <strong>{{ NOTICE_LABELS[raised.notice] }}:</strong>
            {{ raised.headline }}
          </span>
        </p>
        <p v-else class="muted note">{{ activity?.note }}</p>
      </section>
    </div>

    <div class="columns">
      <section v-if="events.length" class="card list rise">
        <h2 class="muted">
          <AppIcon class="lead" name="calendar-clock" />
          <RouterLink class="open" to="/sky">
            Events in the sky
            <AppIcon name="chevron-right" />
          </RouterLink>
        </h2>

        <ol class="events">
          <li
            v-for="event in events"
            :key="`${event.kind}-${event.title}-${event.at}`"
            :class="[event.kind, { gone: passed(event.at) }]"
          >
            <span
              v-if="event.planets?.length"
              :class="['sign', { pair: event.planets.length > 1 }]"
            >
              <AppIcon
                v-for="planet in event.planets"
                :key="planet"
                :name="planetIcon(planet)"
                :scale="planetScale(planet)"
                :style="{ color: planetColor(planet) }"
              />
            </span>
            <EclipseDial
              v-else-if="event.kind === 'eclipse' && event.magnitude !== undefined"
              :label="event.title"
              :magnitude="event.magnitude"
              :solar="event.solar ?? false"
              class="sign"
            />
            <span
              v-else-if="event.kind === 'moon' && event.illumination !== undefined"
              class="sign"
            >
              <MoonDial
                :hemisphere="hemisphere"
                :illumination="event.illumination"
                :label="event.title"
                :size="17"
                waxing
              />
            </span>
            <AppIcon
              v-else
              :name="EVENT_ICONS[event.kind]"
              :style="{ color: eventColor(event, hemisphere) }"
              class="sign"
            />
            <time :datetime="event.at" class="at">
              <span class="day">{{ when(event.at) }}</span>
              <span class="muted hour">{{ event.time_label ?? timeOf(event.at) }}</span>
            </time>
            <span class="what">
              <span class="title">{{ event.title }}</span>
              <span v-if="eventDetail(event, hemisphere)" class="muted detail">
                {{ eventDetail(event, hemisphere) }}
              </span>
            </span>
            <span class="muted away">{{ countdown(event.at) }}</span>
          </li>
        </ol>
      </section>

      <section v-if="launches.length" class="card list rise">
        <h2 class="muted">
          <AppIcon class="lead" name="launch" />
          <RouterLink class="open" to="/launches">
            Rocket launches
            <AppIcon name="chevron-right" />
          </RouterLink>
        </h2>

        <ol class="events">
          <li
            v-for="launch in launches"
            :key="launch.id"
            :class="{ gone: passed(launch.net) && !imminent(launch), soon: imminent(launch) }"
            class="launch"
          >
            <AppIcon
              :class="launchTone(launch)"
              :name="launchMark(launch, clockNow)"
              class="sign"
            />
            <time :datetime="launch.net" class="at">
              <span class="day">{{ when(launch.net) }}</span>
              <span class="muted hour">{{ clockOf(launch) }}</span>
            </time>
            <span class="what">
              <RouterLink :to="`/launches/${launch.id}`" class="title link">
                {{ launch.name }}
                <AppIcon name="chevron-right" />
              </RouterLink>

              <span v-if="launchDetail(launch)" class="muted detail">{{
                launchDetail(launch)
              }}</span>
            </span>
            <span class="muted away">{{ countdown(launch.net) }}</span>
          </li>
        </ol>
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
  padding: var(--space-4) var(--space-5) var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.panels > :nth-child(2),
.columns > :nth-child(2) {
  --rise-delay: 60ms;
}

.panels > :nth-child(3) {
  --rise-delay: 120ms;
}

h2 {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.07em;
  font-weight: 600;
}

h2 .lead {
  font-size: 1.15em;
  color: var(--accent);
}

.open {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  color: inherit;
  text-decoration: none;
  transition: color var(--dur-fast) var(--ease-out);
}

.open .icon {
  transition: transform var(--dur-fast) var(--ease-out);
}

.open:hover,
.open:focus-visible {
  color: var(--accent);
}

.open:hover .icon {
  transform: translateX(2px);
}

.moon-row {
  gap: var(--space-4);
  flex-wrap: nowrap;
}

.moon-facts {
  min-width: 0;
}

.phase {
  margin: 0;
  font-size: var(--text-lg);
  font-weight: 620;
  letter-spacing: -0.02em;
  line-height: 1.25;
  text-wrap: balance;
}

.lit {
  margin: 0;
  font-size: var(--text-sm);
  font-variant-numeric: tabular-nums;
}

.drift {
  white-space: nowrap;
}

.drift .icon {
  font-size: 0.75em;
  margin-right: var(--space-0);
  color: var(--accent);
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
}

.facts dd span {
  font-size: var(--text-sm);
}

.note {
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.planets {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.planets li {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  font-size: var(--text-sm);
}

.planet-mark {
  align-self: center;
  font-size: 1.45rem;
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
  display: inline-flex;
  align-items: center;
  gap: var(--space-0);
  font-size: var(--text-sm);
  white-space: nowrap;
  color: var(--text-muted);
}

.where .icon {
  font-size: 1.05em;
}

.where.dusk {
  color: var(--dusk);
}

.where.dawn {
  color: var(--dawn);
}

.where.all-night {
  color: var(--night);
}

.mag {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-variant-numeric: tabular-nums;
  font-size: var(--text-sm);
  min-width: 4.4ch;
  justify-content: flex-end;
}

.mag .magnitude {
  font-size: 1.05em;
}

.stormy {
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
}

.scope {
  margin: 0;
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.bands {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-size: var(--text-sm);
}

.bands li {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
}

.mark {
  min-width: 2.6rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  padding: 0 var(--space-2);
  text-align: center;
  font-size: var(--text-xs);
  font-weight: 600;
}

.bands li.up .mark {
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  background: color-mix(in srgb, var(--accent) 16%, transparent);
  color: var(--accent);
}

.band-name {
  font-size: var(--text-sm);
}

.raised {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.raised .icon {
  color: var(--accent);
  font-size: 0.9em;
}

.columns {
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.list {
  padding: var(--space-4) var(--space-5) var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
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
  padding: 0 var(--space-1) 0 0;
  display: flex;
  flex-direction: column;
  max-height: 28rem;
  overflow-y: auto;
}

.events li {
  display: grid;
  grid-template-columns: 2rem 5.5rem minmax(0, 1fr) auto;
  align-items: baseline;
  gap: var(--space-3);
  padding: var(--space-2) 0;
  border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
}

.events li:first-child {
  border-top: none;
}

.events li.soon {
  border-left: 2px solid var(--accent);
  padding-left: var(--space-2);
}

.soon .icon {
  color: var(--accent);
}

.gone {
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

.events .sign {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  font-size: 1.3rem;
  justify-self: center;
}

.events .pair .icon {
  font-size: 1.15rem;
}

.events .pair .icon + .icon {
  margin-left: -0.35rem;
}

.events .sign.lost {
  color: var(--bad);
}

.events .sign.flown {
  color: var(--good);
}

.events .sign.ahead {
  color: var(--accent);
}

.at {
  display: flex;
  flex-direction: column;
  line-height: 1.3;
}

.day {
  font-size: var(--text-sm);
  font-weight: 550;
  white-space: nowrap;
}

.hour {
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
}

.what {
  display: flex;
  flex-direction: column;
  min-width: 0;
  line-height: 1.35;
}

.title {
  font-size: var(--text-md);
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

a.title .icon {
  font-size: 0.8em;
  opacity: 0.55;
  vertical-align: 0.05em;
  margin-left: var(--space-0);
}

a.title:hover .icon {
  opacity: 1;
}

.detail {
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.away {
  font-size: var(--text-sm);
  white-space: nowrap;
}

@container (max-width: 30rem) {
  .events li {
    grid-template-columns: 1.6rem minmax(0, 1fr) auto;
    row-gap: var(--space-0);
    column-gap: var(--space-2);
    align-items: start;
  }

  .events .sign {
    grid-column: 1;
    grid-row: 1 / span 2;
    padding-top: var(--space-1);
  }

  .at {
    grid-column: 2;
    grid-row: 1;
    flex-direction: row;
    align-items: baseline;
    gap: var(--space-2);
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
