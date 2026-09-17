<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import ArchiveStrip from '@/components/ArchiveStrip.vue'
import EclipseDial from '@/components/EclipseDial.vue'
import HintPopover from '@/components/HintPopover.vue'
import MagnitudeMark from '@/components/MagnitudeMark.vue'
import MoonDial from '@/components/MoonDial.vue'
import RangeTrack from '@/components/RangeTrack.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import type { EclipseEvent, Planet, Sky, SkyEventKind, SkyOnDate, SkyStrip } from '@/api/types'
import { useArrowKeys } from '@/composables/useArrowKeys'
import { useAsync } from '@/composables/useAsync'
import { usePreferences } from '@/composables/usePreferences'
import { clampDate, formatDate, localDay, localMidnight, nextDay, previousDay } from '@/utils/date'
import {
  EVENT_ICONS,
  eventColor,
  eventDetail,
  planetColor,
  planetIcon,
  planetScale,
  seasonColor,
  seasonOpened,
  seasonProgress,
  VISIBILITY,
} from '@/utils/sky'
import { pageTitle, setTitle } from '@/utils/title'

const FIRST_YEAR = 1900
const LAST_YEAR = 2100
const EARLIEST = `${FIRST_YEAR}-01-01`
const LATEST = `${LAST_YEAR}-12-31`

const DAY = new Intl.DateTimeFormat(undefined, { day: '2-digit', month: 'short', year: 'numeric' })
const TIME = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' })

const route = useRoute()
const router = useRouter()
const { hemisphere } = usePreferences()

const dated = computed(() => (route.params.date ? String(route.params.date) : null))

const {
  data: sky,
  error,
  loading,
  run,
} = useAsync<Sky | SkyOnDate>((signal) => {
  const on = dated.value
  return on ? api.skyOn(on, signal) : api.sky(signal)
})

const strips = ref<SkyStrip[]>([])
api
  .skyStrips()
  .then((found) => (strips.value = found))
  .catch(() => (strips.value = []))

const now = computed(() => (sky.value ? new Date(sky.value.at) : null))
const here = computed(() => dated.value ?? sky.value?.at.slice(0, 10) ?? null)
const picked = ref<Date | null>(null)

function day(iso: string): string {
  const at = new Date(iso)
  return Number.isNaN(at.getTime()) ? iso : DAY.format(at)
}

function clock(iso: string): string {
  const at = new Date(iso)
  return Number.isNaN(at.getTime()) ? '' : TIME.format(at)
}

function thousands(value: number): string {
  return Math.round(value).toLocaleString()
}

function signed(magnitude: number): string {
  return `${magnitude < 0 ? '−' : '+'}${Math.abs(magnitude).toFixed(1)}`
}

function capitalized(word: string): string {
  return word.charAt(0).toUpperCase() + word.slice(1)
}

const season = computed(() =>
  sky.value ? seasonProgress(sky.value.season.at, sky.value.next_turning.at, sky.value.at) : null,
)

const seasonNow = computed(() =>
  sky.value ? seasonColor(seasonOpened(sky.value.season, hemisphere.value)) : 'var(--accent)',
)
const seasonNext = computed(() =>
  sky.value ? seasonColor(seasonOpened(sky.value.next_turning, hemisphere.value)) : 'var(--accent)',
)
const seasonFill = computed(() => `linear-gradient(90deg, ${seasonNow.value}, ${seasonNext.value})`)

const seasonLine = computed(() => {
  const turning = sky.value?.season
  if (!turning) return ''
  return hemisphere.value === 'south'
    ? `${capitalized(turning.opens_southern)} in the south, ${turning.opens_northern} in the north`
    : `${capitalized(turning.opens_northern)} in the north, ${turning.opens_southern} in the south`
})

function sight(planet: Planet): { icon: string; title: string } {
  if (planet.visibility === 'lost') return { icon: 'eye-off', title: 'too close to the sun to see' }
  if (planet.naked_eye) return { icon: 'eye', title: 'visible to the naked eye' }
  return { icon: 'telescope', title: 'needs a telescope' }
}

function eclipseNote(found: EclipseEvent): string {
  if (found.solar) return 'Along its own track'
  if (/penumbral/i.test(found.label)) return 'A slight dimming, seen from the whole night side'
  return 'Whole night side of Earth'
}

function stripsFor(kind: SkyEventKind): SkyStrip[] {
  return strips.value.filter((strip) => strip.kind === kind && strip.entries > 0)
}

function go(date: string) {
  void router.push(`/sky/${clampDate(date, EARLIEST, LATEST)}`)
}

function step(by: -1 | 1) {
  const from = here.value
  if (!from) return

  const target = by < 0 ? previousDay(from) : nextDay(from)
  if (target) go(target)
}

useArrowKeys({ left: () => step(-1), right: () => step(1) })

watch(picked, (chosen) => {
  if (chosen) go(localDay(chosen))
})

watch(
  dated,
  (on) => {
    picked.value = on ? localMidnight(on) : null
    setTitle(on ? pageTitle(`The sky on ${formatDate(on)}`) : pageTitle('The sky today'))
    void run()
  },
  { immediate: true },
)
</script>

<template>
  <div class="stack sky">
    <header class="row head">
      <h1>{{ dated ? `The sky on ${formatDate(dated)}` : 'The sky today' }}</h1>
      <HintPopover label="About this page">
        <p>
          Positions come from mean orbital elements and the Espenak and Meeus delta-T polynomials
          between 1900 and 2100. Spotted something wrong?
          <RouterLink to="/contact">Tell me</RouterLink>.
        </p>
      </HintPopover>

      <div class="row picker">
        <RouterLink
          :aria-hidden="!dated"
          :class="['back', { away: !dated }]"
          :tabindex="dated ? undefined : -1"
          to="/sky"
        >
          Tonight
        </RouterLink>
        <Button aria-label="The day before" rounded severity="secondary" text @click="step(-1)">
          <template #icon><AppIcon name="chevron-left" /></template>
        </Button>
        <DatePicker
          v-model="picked"
          :max-date="localMidnight(LATEST)"
          :min-date="localMidnight(EARLIEST)"
          date-format="yy-mm-dd"
          placeholder="Any date"
          show-icon
          size="small"
        />
        <Button aria-label="The day after" rounded severity="secondary" text @click="step(1)">
          <template #icon><AppIcon name="chevron-right" /></template>
        </Button>
      </div>
    </header>

    <RetryNotice v-if="error" :busy="loading" :message="error" @retry="run" />

    <div v-else-if="loading && !sky" class="stack">
      <Skeleton height="11rem" width="100%" />
      <Skeleton height="16rem" width="100%" />
    </div>

    <template v-else-if="sky && now">
      <div class="panels">
        <section class="card panel moon rise">
          <h2 class="muted">
            <AppIcon name="moon" />
            The moon
          </h2>

          <div class="dial">
            <MoonDial
              :hemisphere="hemisphere"
              :illumination="sky.moon.illumination"
              :label="sky.moon.label"
              :waxing="sky.moon.waxing"
            />
            <div class="readings">
              <p class="phase">{{ sky.moon.label }}</p>
              <p class="muted">
                {{ Math.round(sky.moon.illumination * 100) }}% lit &middot;
                {{ sky.moon.age_days.toFixed(1) }} days into the cycle
              </p>
              <p class="muted">
                {{ thousands(sky.moon.distance_km) }} km away &middot;
                {{ sky.moon.closing ? 'coming closer' : 'moving away' }}
              </p>
            </div>
          </div>

          <RangeTrack
            :max="sky.moon.apogee_km"
            :max-label="`${thousands(sky.moon.apogee_km)} km`"
            :min="sky.moon.perigee_km"
            :min-label="`${thousands(sky.moon.perigee_km)} km`"
            :value="sky.moon.distance_km"
            max-note="apogee"
            min-note="perigee"
          />

          <dl class="facts">
            <div v-for="quarter in sky.moon.next_quarters" :key="quarter.quarter">
              <dt class="muted">{{ quarter.label }}</dt>
              <dd>{{ day(quarter.at) }} &middot; {{ clock(quarter.at) }}</dd>
            </div>
            <div v-for="apside in sky.moon.next_apsides" :key="apside.apside">
              <dt class="muted">{{ apside.label }}</dt>
              <dd>{{ day(apside.at) }} &middot; {{ thousands(apside.distance_km) }} km</dd>
            </div>
          </dl>

          <ArchiveStrip :strips="stripsFor('moon')" />
        </section>

        <section v-if="sky.planets.length" class="card panel rise">
          <h2 class="muted">
            <AppIcon name="planets" />
            Planet observability
            <HintPopover label="About the planets panel">
              <p>
                <strong>Magnitude</strong> is the apparent brightness where lower is brighter. Venus
                reaches -4, the faintest naked-eye stars are at about 6.
              </p>
              <p>
                <strong>AU</strong> is an Astronomical Unit, the distance between Earth and Sun
                which is about 150 million km.
              </p>
              <p>
                <strong>Opposition</strong> is when a planet sits opposite of the sun, so it is up
                all night and at its brightest. <strong>Greatest elongation</strong> is when Mercury
                or Venus are positioned as far away from the sun as they can, which is the best time
                to observe them.
              </p>
              <p>
                Beside the magnitude, an <strong>eye</strong> marks a planet the naked eye can find,
                a <strong>telescope</strong> one that needs help, and a crossed eye one too close to
                the sun to see at all.
              </p>
              <p>
                Whether a planet is actually over the horizon for you at a given hour depends on
                your location.
              </p>
            </HintPopover>
          </h2>

          <p aria-hidden="true" class="muted heads rows-heads">
            <span>{{ dated ? 'That day' : 'Today' }}</span>
            <span>Next best view</span>
          </p>

          <ul class="rows marked scroller">
            <li v-for="planet in sky.planets" :key="planet.planet" :class="planet.visibility">
              <AppIcon
                :name="planetIcon(planet.planet)"
                :scale="planetScale(planet.planet)"
                :style="{ color: planetColor(planet.planet) }"
                class="sign"
              />
              <span class="what">
                <span class="title">
                  {{ planet.name }}
                  <span class="muted au">{{ planet.distance_au.toFixed(2) }} AU</span>
                </span>
                <span class="muted detail bits spaced">
                  <span :class="['bit', 'window', VISIBILITY[planet.visibility].tone]">
                    <AppIcon :name="VISIBILITY[planet.visibility].icon" />
                    {{ planet.visibility_label }}
                  </span>
                  <span
                    :title="`Apparent magnitude ${signed(planet.magnitude)}, ${sight(planet).title}`"
                    class="bit mag"
                  >
                    <MagnitudeMark :magnitude="planet.magnitude" />
                    {{ signed(planet.magnitude) }}
                    <AppIcon
                      :label="sight(planet).title"
                      :name="sight(planet).icon"
                      class="sight"
                    />
                  </span>
                </span>
              </span>
              <span v-if="planet.next_milestone" class="away">
                <span class="muted">{{ day(planet.next_milestone.at) }}</span>
                <span :title="planet.next_milestone.label" class="muted note">
                  {{ planet.next_milestone.short }}
                </span>
              </span>
            </li>
          </ul>

          <ArchiveStrip :strips="stripsFor('planet')" />
        </section>

        <section v-if="sky.conjunctions.length" class="card panel rise">
          <h2 class="muted">
            <AppIcon name="conjunction" />
            Conjunctions
            <HintPopover label="About conjunctions">
              <p>
                A <strong>conjunction</strong> is when two planets appear close to each other in the
                night sky.
              </p>
            </HintPopover>
          </h2>

          <p aria-hidden="true" class="muted heads rows-heads">
            <span>Pair</span>
            <span>Closest at</span>
          </p>

          <ul class="rows marked scroller">
            <li v-for="meet in sky.conjunctions" :key="`${meet.names.join()}-${meet.at}`">
              <span class="sign pair">
                <AppIcon
                  v-for="planet in meet.planets"
                  :key="planet"
                  :name="planetIcon(planet)"
                  :scale="planetScale(planet)"
                  :style="{ color: planetColor(planet) }"
                />
              </span>
              <span class="what">
                <span class="title">{{ meet.names[0] }} and {{ meet.names[1] }}</span>
                <span class="muted detail bits spaced">
                  <span class="bit">{{ meet.separation.toFixed(1) }}&deg; apart</span>
                  <span :class="['bit', 'window', VISIBILITY[meet.visibility].tone]">
                    <AppIcon :name="VISIBILITY[meet.visibility].icon" />
                    {{ meet.visibility_label }}
                  </span>
                </span>
              </span>
              <span class="muted away">{{ day(meet.at) }}</span>
            </li>
          </ul>

          <ArchiveStrip :strips="stripsFor('conjunction')" />
        </section>

        <section v-if="sky.showers.length" class="card panel rise">
          <h2 class="muted">
            <AppIcon name="shower" />
            Meteor showers
            <HintPopover label="About meteor showers">
              <p>
                Earth passes through dust a comet leaves behind, and the grains burn up as they hit
                the atmosphere.
              </p>
              <p>
                The stated <strong>rate</strong> is the zenithal hourly rate which is the rate at
                peak time and perfect weather conditions. The real rate for you might be lower.
              </p>
              <p>A bright moon is able to wash out all but the brightest shooting stars.</p>
            </HintPopover>
          </h2>

          <p aria-hidden="true" class="muted heads rows-heads">
            <span>Shower</span>
            <span>Peak night</span>
          </p>

          <ul class="rows scroller">
            <li v-for="shower in sky.showers" :key="shower.name">
              <span class="what">
                <span class="title">{{ shower.name }}</span>
                <span class="muted detail">
                  {{ shower.zenith_hourly_rate }} an hour from {{ shower.radiant }} &middot;
                  {{ shower.parent }}
                </span>
              </span>
              <span class="away">
                <span class="muted">{{ day(shower.peak) }}</span>
                <span :class="['note', shower.moonlight]" :title="shower.moonlight_label">
                  {{ shower.moonlight_short }}
                </span>
              </span>
            </li>
          </ul>

          <ArchiveStrip :strips="stripsFor('shower')" />
        </section>

        <section v-if="sky.eclipses.length" class="card panel rise">
          <h2 class="muted">
            <AppIcon name="eclipse" />
            Eclipses
            <HintPopover label="About eclipses">
              <p>
                A <strong>solar eclipse</strong> is the moon passing in front of the sun, visible
                only along a narrow track. A <strong>lunar eclipse</strong> is the moon entering
                Earth's shadow, visible from the whole night side at once.
              </p>
              <p>
                <strong>Magnitude</strong> here means how much of the sun or moon is covered, above
                1 means total.
              </p>
              <p>
                A <strong>penumbral</strong> lunar eclipse only crosses the lighter, outer part of
                Earth's shadow, so the moon dims a little instead of showing a bite. Its magnitude
                is how far into that outer shadow it goes.
              </p>
            </HintPopover>
          </h2>

          <p aria-hidden="true" class="muted heads rows-heads">
            <span>Eclipse</span>
            <span>When</span>
          </p>

          <ul class="rows marked scroller">
            <li v-for="found in sky.eclipses" :key="found.at">
              <EclipseDial
                :label="found.label"
                :magnitude="found.magnitude"
                :solar="found.solar"
                class="sign"
              />
              <span class="what">
                <span class="title">{{ found.label }}</span>
                <span class="muted detail">
                  {{ eclipseNote(found) }} &middot; magnitude {{ found.magnitude.toFixed(2) }}
                </span>
              </span>
              <span class="muted away">{{ day(found.at) }} {{ clock(found.at) }}</span>
            </li>
          </ul>

          <ArchiveStrip :strips="stripsFor('eclipse')" />
        </section>

        <section class="card panel earth rise">
          <h2 class="muted">
            <AppIcon name="orbit" />
            Earth's orbit
            <HintPopover label="About Earth's orbit">
              <p>
                An <strong>equinox</strong> is one of the two days a year when the sun crosses the
                equator and day and night are near equal everywhere. A <strong>solstice</strong> is
                the longest or shortest day, when the sun is as far north or south as it gets.
              </p>
              <p>
                <strong>Perihelion</strong> is Earth's closest point to the sun and
                <strong>aphelion</strong> the farthest. The gap is only 3%, and northern winter
                falls near the closest point, so the seasons come from the tilt and not the
                distance.
              </p>
            </HintPopover>
          </h2>

          <div v-if="season" class="season">
            <p class="season-line">
              <span :style="{ color: seasonNow }" class="season-name">{{ seasonLine }}</span>
              <span class="muted season-left">
                {{ Math.round(season.fraction * 100) }}% through &middot; {{ season.daysLeft }}
                {{ season.daysLeft === 1 ? 'day' : 'days' }} left
              </span>
            </p>
            <RangeTrack
              :fill="seasonFill"
              :max="1"
              :max-label="day(sky.next_turning.at)"
              :max-note="sky.next_turning.label"
              :min="0"
              :min-label="day(sky.season.at)"
              :min-note="sky.season.label"
              :pin="seasonNow"
              :value="season.fraction"
            />
          </div>

          <dl class="facts">
            <div v-for="apsis in sky.earth_apsides" :key="apsis.apsis">
              <dt class="muted">{{ apsis.label }}</dt>
              <dd>{{ day(apsis.at) }} &middot; {{ apsis.distance_au.toFixed(4) }} AU</dd>
            </div>
          </dl>

          <ArchiveStrip :strips="stripsFor('season')" />
        </section>
      </div>

      <section v-if="sky.events.length" class="card panel wide rise">
        <h2 class="muted">
          <AppIcon name="calendar-clock" />
          Upcoming events
        </h2>

        <p aria-hidden="true" class="muted heads timeline-heads">
          <span></span>
          <span>When</span>
          <span>What</span>
        </p>

        <ol class="timeline scroller">
          <li
            v-for="event in sky.events"
            :key="`${event.kind}-${event.title}-${event.at}`"
            :class="[event.kind, { gone: new Date(event.at) < now }]"
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
            <time :datetime="event.at" class="when">
              <span class="date">{{ day(event.at) }}</span>
              <span class="hour">{{ event.time_label ?? clock(event.at) }}</span>
            </time>
            <span class="what">
              <span class="title">{{ event.title }}</span>
              <span v-if="eventDetail(event, hemisphere)" class="muted detail">
                {{ eventDetail(event, hemisphere) }}
              </span>
            </span>
          </li>
        </ol>
      </section>
    </template>
  </div>
</template>

<style scoped>
.sky {
  gap: var(--space-4);
}

.head {
  gap: var(--space-3);
}

h1 {
  font-size: var(--text-xl);
}

.picker {
  gap: var(--space-1);
  margin-left: auto;
}

.picker .back {
  font-size: var(--text-xs);
  white-space: nowrap;
}

.picker .back.away {
  visibility: hidden;
}

.panels {
  display: grid;
  gap: var(--space-4);
  grid-template-columns: minmax(0, 1fr);
  align-items: stretch;
}

.scroller {
  scrollbar-width: thin;
}

.panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-5);
}

.panel h2 {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-xs);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.dial {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  flex-wrap: wrap;
}

.readings {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.readings p {
  margin: 0;
  font-size: var(--text-sm);
}

.phase {
  font-size: var(--text-lg);
}

.facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 12rem), 1fr));
  gap: var(--space-2) var(--space-4);
  margin: 0;
}

.facts dt {
  font-size: var(--text-2xs);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.facts dd {
  margin: 0;
  font-size: var(--text-sm);
  font-variant-numeric: tabular-nums;
}

.rows,
.timeline {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}

.heads {
  display: grid;
  margin: 0 0 calc(var(--space-2) * -1);
  font-size: var(--text-2xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.rows-heads {
  grid-template-columns: minmax(0, 1fr) minmax(0, 8rem);
  gap: var(--space-3);
}

.rows-heads span:last-child {
  text-align: right;
}

.timeline-heads {
  grid-template-columns: var(--mark-col) 8.5rem minmax(0, 1fr);
  gap: 0 var(--space-3);
}

.panels,
.wide {
  --mark-col: 2rem;
}

.sign {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  align-self: center;
  width: var(--mark-col);
  font-size: 1.5rem;
  color: var(--text-muted);
}

.panels > :nth-child(2) {
  --rise-delay: 50ms;
}

.panels > :nth-child(3) {
  --rise-delay: 100ms;
}

.panels > :nth-child(4) {
  --rise-delay: 150ms;
}

.panels > :nth-child(5) {
  --rise-delay: 200ms;
}

.panels > :nth-child(6) {
  --rise-delay: 250ms;
}

.wide {
  --rise-delay: 300ms;
}

.pair .icon {
  font-size: 1.3rem;
}

.pair .icon + .icon {
  margin-left: -0.35rem;
}

.rows.marked li {
  grid-template-columns: var(--mark-col) minmax(0, 1fr) minmax(0, 8rem);
}

.au {
  margin-left: var(--space-1);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.bits {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  column-gap: var(--space-1);
}

.bit {
  display: inline-flex;
  align-items: center;
  gap: var(--space-0);
  white-space: nowrap;
}

.bit + .bit::before {
  content: '·';
  margin-right: var(--space-1);
  color: var(--text-muted);
}

.bits.spaced {
  column-gap: var(--space-4);
}

.bits.spaced .bit + .bit::before {
  content: none;
  margin: 0;
}

.window .icon {
  font-size: 1.1em;
}

.window.dusk {
  color: var(--dusk);
}

.window.dawn {
  color: var(--dawn);
}

.window.all-night {
  color: var(--night);
}

.mag {
  gap: var(--space-1);
  font-variant-numeric: tabular-nums;
}

.mag .magnitude {
  font-size: 1.1em;
}

.mag .sight {
  margin-left: var(--space-1);
  font-size: 1.15em;
  opacity: 0.85;
}

.season {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.season-line {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-2) var(--space-3);
  margin: 0;
  font-size: var(--text-sm);
  flex-wrap: wrap;
}

.season-name {
  font-weight: 550;
}

.season-left {
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.rows li.lost {
  opacity: 0.62;
}

.rows li,
.timeline li {
  display: grid;
  align-items: center;
  gap: 0 var(--space-3);
  padding-block: var(--space-1);
  border-top: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
}

.rows li {
  grid-template-columns: minmax(0, 1fr) minmax(0, 8rem);
}

.timeline li {
  grid-template-columns: var(--mark-col) 8.5rem minmax(0, 1fr);
}

.timeline .sign {
  font-size: 1.3rem;
}

.timeline .gone {
  opacity: 0.5;
}

.when {
  display: flex;
  flex-direction: column;
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
  color: var(--text-muted);
  white-space: nowrap;
}

.when .hour {
  min-height: 1em;
  opacity: 0.75;
}

.what {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.title {
  font-size: var(--text-sm);
}

.detail,
.away {
  font-size: var(--text-xs);
}

.away {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  text-align: right;
  text-wrap: balance;
}

.away .note {
  font-size: var(--text-2xs);
  white-space: nowrap;
  color: var(--text-muted);
  opacity: 0.8;
}

.away .note.dark {
  color: var(--good);
  opacity: 1;
}

.away .note.some {
  color: hsl(var(--tone-raised));
  opacity: 1;
}

.away .note.washed_out {
  color: var(--bad);
  opacity: 1;
}

@media (min-width: 58rem) {
  .panels {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .panel:has(.scroller) {
    max-height: 26rem;
  }

  .wide {
    max-height: 40rem;
  }

  .timeline .what {
    display: grid;
    grid-template-columns: minmax(0, 17rem) minmax(0, 1fr);
    gap: 0 var(--space-4);
    align-items: baseline;
  }

  .scroller {
    flex: 1;
    min-height: 6rem;
    overflow-y: auto;
  }
}

@media (max-width: 44rem) {
  .head {
    flex-wrap: wrap;
  }

  .picker {
    order: 1;
    width: 100%;
    flex-wrap: wrap;
    justify-content: center;
  }

  .picker .back {
    order: 2;
    width: 100%;
    text-align: center;
  }

  .picker .back.away {
    display: none;
  }

  .picker :deep(.p-datepicker) {
    flex: 1;
    min-width: 0;
  }

  h1 {
    font-size: var(--text-lg);
  }

  .rows-heads {
    display: none;
  }

  .timeline-heads {
    grid-template-columns: var(--mark-col) 6.5rem minmax(0, 1fr);
  }

  .rows li {
    grid-template-columns: minmax(0, 1fr);
  }

  .rows.marked li {
    grid-template-columns: var(--mark-col) minmax(0, 1fr);
  }

  .rows.marked .sign {
    grid-row: 1 / span 2;
    align-self: start;
    margin-top: var(--space-0);
  }

  .rows.marked .away {
    grid-column: 2;
  }

  .away {
    text-wrap: pretty;
  }

  .away {
    align-items: flex-start;
    text-align: left;
  }

  .timeline li {
    grid-template-columns: var(--mark-col) 6.5rem minmax(0, 1fr);
  }
}
</style>
