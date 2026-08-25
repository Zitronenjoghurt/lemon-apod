<script lang="ts" setup>
import { computed, onMounted, onUnmounted, ref, useTemplateRef, watch } from 'vue'
import RetryNotice from '@/components/RetryNotice.vue'
import ScaleKey from '@/components/ScaleKey.vue'
import SeriesChart, { type SeriesPoint } from '@/components/SeriesChart.vue'
import { api } from '@/api/client'
import type { NoticeKind, ScaleDay, WeatherBand } from '@/api/types'
import { useAsync } from '@/composables/useAsync'
import { useNarrow } from '@/composables/useNarrow'
import {
  BAND_ABOUT,
  BAND_NAMES,
  BANDS,
  DST_BANDS,
  DST_FRAME,
  DST_MARKS,
  DST_TICKS,
  FLUX_FRAME,
  FLUX_MARKS,
  FLUX_TICKS,
  inForce,
  KP_BANDS,
  KP_FRAME,
  KP_TICKS,
  kpPercent,
  kpReading,
  kpTone,
  levelName,
  levelOdds,
  levelWord,
  NOTICE_ICONS,
  NOTICE_LABELS,
  SCALES_URL,
  SWPC_URL,
} from '@/utils/weather'

const DAY = new Intl.DateTimeFormat(undefined, { weekday: 'short', day: 'numeric', month: 'short' })
const HOUR = new Intl.DateTimeFormat(undefined, { day: 'numeric', month: 'short', hour: 'numeric' })
const STAMP = new Intl.DateTimeFormat(undefined, {
  day: 'numeric',
  month: 'short',
  hour: 'numeric',
  minute: '2-digit',
})

const PER_PAGE = 8

const KINDS: { label: string; value: NoticeKind | 'all' }[] = [
  { label: 'All', value: 'all' },
  { label: 'Alerts', value: 'alert' },
  { label: 'Warnings', value: 'warning' },
  { label: 'Watches', value: 'watch' },
  { label: 'Summaries', value: 'summary' },
]

const { data: report, error, notFound, loading, run } = useAsync((signal) => api.weather(signal))
const { pageLinks } = useNarrow()

const opened = ref<string>()
const about = ref<WeatherBand>()

const kind = ref<NoticeKind | 'all'>('all')
const page = ref(1)

const roomyQuery = window.matchMedia('(min-width: 38rem)')
const roomy = ref(roomyQuery.matches)

function onRoomy(event: MediaQueryListEvent): void {
  roomy.value = event.matches
}

onMounted(() => {
  roomyQuery.addEventListener('change', onRoomy)
  run()
})

onUnmounted(() => roomyQuery.removeEventListener('change', onRoomy))

const now = computed(() => (report.value ? kpReading(report.value.kp) : null))
const dial = computed(() => (report.value ? kpPercent(report.value.kp) : 0))
const measuredAt = computed(() => stamp(report.value?.observed_at))
const days = computed<ScaleDay[]>(() => report.value?.outlook ?? [])
const observed = computed(() => report.value?.scales)

const kpPoints = computed<SeriesPoint[]>(
  () =>
    report.value?.kp_series.map((point) => ({
      label: hour(point.at),
      value: point.kp,
      ahead: point.ahead,
      tone: kpTone(point.kp),
    })) ?? [],
)

const fluxPoints = computed<SeriesPoint[]>(
  () => report.value?.flux.map((point) => ({ label: day(point.at), value: point.flux })) ?? [],
)

const dstPoints = computed<SeriesPoint[]>(
  () => report.value?.dst.map((point) => ({ label: hour(point.at), value: point.dst })) ?? [],
)

const forecastFrom = computed(() => report.value?.kp_series.find((point) => point.ahead)?.at)

const notices = computed(() => report.value?.alerts ?? [])
const raised = computed(() => notices.value.filter((alert) => inForce(alert)))

const kinds = computed(() => {
  const counts = new Map<NoticeKind, number>()
  for (const alert of notices.value) {
    counts.set(alert.notice, (counts.get(alert.notice) ?? 0) + 1)
  }

  return KINDS.filter((one) => one.value === 'all' || counts.has(one.value)).map((one) => ({
    ...one,
    count: one.value === 'all' ? notices.value.length : (counts.get(one.value) ?? 0),
  }))
})

const filtered = computed(() =>
  kind.value === 'all' ? notices.value : notices.value.filter((one) => one.notice === kind.value),
)

const shown = computed(() =>
  filtered.value.slice((page.value - 1) * PER_PAGE, page.value * PER_PAGE),
)

const picked = computed(() => kinds.value.find((one) => one.value === kind.value))

function pick(chosen: NoticeKind | 'all' | null): void {
  kind.value = chosen ?? 'all'
}

function onPage(event: { page: number }): void {
  page.value = event.page + 1
  opened.value = undefined
}

watch([kind, filtered], () => {
  page.value = 1
  opened.value = undefined
})

function format(value: string | undefined, shape: Intl.DateTimeFormat): string {
  if (!value) return ''
  const at = new Date(value)
  return Number.isNaN(at.getTime()) ? value : shape.format(at)
}

const day = (value: string) => format(value, DAY)
const hour = (value: string) => format(value, HOUR)
const stamp = (value: string | undefined) => format(value, STAMP)

function dayName(date: string): string {
  const at = Date.parse(`${date}T12:00:00Z`)
  if (Number.isNaN(at)) return date

  const here = new Date()
  const midday = Date.UTC(here.getFullYear(), here.getMonth(), here.getDate(), 12)
  const away = Math.round((at - midday) / 86_400_000)

  if (away === 0) return 'Today'
  if (away === 1) return 'Tomorrow'
  if (away === -1) return 'Yesterday'
  return format(`${date}T12:00:00Z`, DAY) || date
}

function toggle(id: string): void {
  opened.value = opened.value === id ? undefined : id
}

function explain(band: WeatherBand): void {
  about.value = about.value === band ? undefined : band
}

type Keyed = { toggle: (event: Event) => void }

const kpKey = useTemplateRef<Keyed>('kpKey')
const fluxKey = useTemplateRef<Keyed>('fluxKey')
const dstKey = useTemplateRef<Keyed>('dstKey')
</script>

<template>
  <div class="stack weather">
    <header class="stack head">
      <h1>Space weather</h1>
      <p class="muted blurb">
        Solar activity and its influence on earth, as measured and forecast by NOAA's Space Weather
        Prediction Center.
      </p>
    </header>

    <RetryNotice v-if="error" :busy="loading" :message="error" @retry="run" />

    <p v-else-if="notFound" class="card empty">Nothing has been polled from NOAA yet.</p>

    <div v-else-if="loading && !report" class="stack">
      <Skeleton height="9rem" width="100%" />
      <Skeleton height="14rem" width="100%" />
    </div>

    <template v-else-if="report && now">
      <section class="card panel now">
        <div class="stack reading">
          <p class="muted eyebrow">Planetary K index, measured {{ measuredAt }}</p>
          <p class="kp">
            <span class="figure">{{ report.kp.toFixed(2) }}</span>
            <span class="unit muted">Kp</span>
            <span class="verdict">{{ now.label }}</span>
          </p>

          <div class="gauge">
            <div class="track">
              <div :style="{ width: `${dial}%` }" class="fill" />
              <div class="threshold" />
            </div>
            <p class="muted caption">Scale of 0 to 9. Storms start at 5.</p>
          </div>

          <p class="muted note">{{ now.note }}</p>
        </div>

        <div class="stack right">
          <div v-if="raised.length" class="stack in-force">
            <h2 class="muted">In force now</h2>
            <p v-for="alert in raised" :key="alert.id" class="raised">
              <i :class="['pi', NOTICE_ICONS[alert.notice]]" aria-hidden="true" />
              <span>
                <strong>{{ NOTICE_LABELS[alert.notice] }}:</strong> {{ alert.headline }}
                <span v-if="alert.scale" class="muted scale">{{ alert.scale }}</span>
                <span v-if="alert.valid_until" class="muted until">
                  until {{ stamp(alert.valid_until) }}
                </span>
              </span>
            </p>
          </div>
          <p v-else class="muted quiet">
            <i aria-hidden="true" class="pi pi-check-circle" />
            NOAA has no alert or warning running.
          </p>

          <div v-if="observed" class="stack observed">
            <h2 class="muted">
              Current conditions
              <span v-if="observed?.observed_at" class="stamp">
                as of {{ stamp(observed.observed_at) }}
              </span>
            </h2>
            <ul class="levels">
              <li v-for="level in observed.levels" :key="level.band" class="level">
                <span class="level-name">{{ BAND_NAMES[level.band] }}</span>
                <span :class="['level-value', { up: (level.scale ?? 0) > 0 }]">
                  {{ levelName(level) }}
                  <span v-if="level.scale" class="muted word">{{ levelWord(level) }}</span>
                </span>
              </li>
            </ul>
          </div>
        </div>
      </section>

      <section v-if="days.length" class="card panel">
        <h2 class="muted">NOAA Scales Forecast</h2>

        <div class="table-scroll">
          <table class="scales">
            <thead>
              <tr>
                <th scope="col">Scale</th>
                <th v-for="one in days" :key="one.date" scope="col">
                  {{ dayName(one.date) }}
                </th>
              </tr>
            </thead>
            <tbody>
              <template v-for="band in BANDS" :key="band">
                <tr>
                  <th scope="row">
                    <button
                      :aria-expanded="about === band"
                      :aria-label="`What ${BAND_NAMES[band].toLowerCase()} means`"
                      class="band"
                      type="button"
                      @click="explain(band)"
                    >
                      <span class="letter">{{ band.toUpperCase() }}</span>
                      <span class="muted what">
                        {{ BAND_NAMES[band] }}
                        <i aria-hidden="true" class="pi pi-info-circle hint" />
                      </span>
                    </button>
                  </th>
                  <td v-for="one in days" :key="one.date">
                    <template v-for="level in one.levels" :key="level.band">
                      <span
                        v-if="level.band === band"
                        :class="[
                          'cell',
                          {
                            up: (level.scale ?? 0) > 0,
                            blank: level.scale === null && !levelOdds(level).length,
                          },
                        ]"
                      >
                        {{ levelName(level) }}
                        <span v-if="level.scale" class="muted word">{{ levelWord(level) }}</span>
                        <span v-if="levelOdds(level).length" class="odds-list">
                          <span
                            v-for="odds in levelOdds(level)"
                            :key="odds.of"
                            :class="{ leading: odds.chance >= 50 }"
                            class="odds"
                          >
                            <span class="chance">{{ odds.chance }}%</span>
                            <span class="muted of">{{ odds.of }}</span>
                          </span>
                        </span>
                      </span>
                    </template>
                  </td>
                </tr>
                <tr v-if="about === band" class="about">
                  <td :colspan="days.length + 1" class="muted">{{ BAND_ABOUT[band] }}</td>
                </tr>
              </template>
            </tbody>
          </table>
        </div>

        <p class="muted note">
          <a :href="SCALES_URL" data-ours rel="noopener" target="_blank"
            >NOAA explains the scales here</a
          >.
        </p>
      </section>

      <div class="charts">
        <section v-if="kpPoints.length" class="card panel">
          <h2 class="muted heading">
            Geomagnetic activity
            <button
              :aria-label="`What the geomagnetic activity levels mean`"
              class="key-open"
              type="button"
              @click="kpKey?.toggle($event)"
            >
              <i aria-hidden="true" class="pi pi-question-circle" />
            </button>
          </h2>
          <SeriesChart
            :bands="KP_BANDS"
            :decimals="2"
            :frame="KP_FRAME"
            :points="kpPoints"
            :ticks="KP_TICKS"
            label="Kp, measured and forecast"
          />
          <p class="muted note">
            Solid bars are measured, outlined bars are NOAA's forecast<template v-if="forecastFrom">
              from {{ stamp(forecastFrom) }}</template
            >. Three hours per bar.
          </p>
        </section>

        <section v-if="fluxPoints.length" class="card panel">
          <h2 class="muted heading">
            Solar radio flux
            <button
              :aria-label="`What the solar radio flux levels mean`"
              class="key-open"
              type="button"
              @click="fluxKey?.toggle($event)"
            >
              <i aria-hidden="true" class="pi pi-question-circle" />
            </button>
          </h2>
          <SeriesChart
            :frame="FLUX_FRAME"
            :marks="FLUX_MARKS"
            :points="fluxPoints"
            :ticks="FLUX_TICKS"
            :zeroed="false"
            kind="line"
            label="F10.7, thirty days"
            unit=" sfu"
          />
          <p class="muted note">
            Radio brightness at 10.7cm, the standard measure of how busy the sun is.
          </p>
        </section>

        <section v-if="dstPoints.length" class="card panel">
          <h2 class="muted heading">
            Ring current
            <button
              :aria-label="`What the ring current levels mean`"
              class="key-open"
              type="button"
              @click="dstKey?.toggle($event)"
            >
              <i aria-hidden="true" class="pi pi-question-circle" />
            </button>
          </h2>
          <SeriesChart
            :bands="DST_BANDS"
            :frame="DST_FRAME"
            :marks="DST_MARKS"
            :points="dstPoints"
            :ticks="DST_TICKS"
            :zeroed="false"
            kind="line"
            label="Dst, the last week"
            unit=" nT"
          />
          <p class="muted note">
            How far Earth's field has been pushed out of shape. A storm drives it down as the ring
            current builds, and an arriving shock briefly pushes it the other way.
          </p>
        </section>
      </div>

      <Popover ref="kpKey">
        <ScaleKey :bands="KP_BANDS" />
      </Popover>
      <Popover ref="fluxKey">
        <ScaleKey :marks="FLUX_MARKS" />
      </Popover>
      <Popover ref="dstKey">
        <ScaleKey :bands="DST_BANDS" :marks="DST_MARKS" />
      </Popover>

      <section v-if="notices.length" class="card panel">
        <div class="row notices-head">
          <h2 class="muted">Everything NOAA has issued lately</h2>
          <SelectButton
            v-if="roomy"
            :allow-empty="false"
            :model-value="kind"
            :options="kinds"
            aria-labelledby="kind-label"
            option-value="value"
            size="small"
            @update:model-value="pick"
          >
            <template #option="{ option }">
              <span class="option">
                {{ option.label }}
                <span class="tally">{{ option.count }}</span>
              </span>
            </template>
          </SelectButton>

          <Select
            v-else
            :model-value="kind"
            :options="kinds"
            aria-labelledby="kind-label"
            option-label="label"
            option-value="value"
            size="small"
            @update:model-value="pick"
          >
            <template #value>
              <span class="option">
                {{ picked?.label }}
                <span class="tally">{{ picked?.count }}</span>
              </span>
            </template>
            <template #option="{ option }">
              <span class="option">
                {{ option.label }}
                <span class="tally">{{ option.count }}</span>
              </span>
            </template>
          </Select>
          <span id="kind-label" class="sr-only">Kind of notice</span>
        </div>

        <ul class="notices">
          <li v-for="alert in shown" :key="alert.id" :class="{ live: inForce(alert) }">
            <button :aria-expanded="opened === alert.id" type="button" @click="toggle(alert.id)">
              <i :class="['pi', NOTICE_ICONS[alert.notice]]" aria-hidden="true" />
              <span class="what">
                <span class="line">
                  <strong>{{ NOTICE_LABELS[alert.notice] }}</strong>
                  {{ alert.headline }}
                </span>
                <span class="muted when">
                  {{ stamp(alert.issued_at) }}
                  <template v-if="alert.scale"> &middot; {{ alert.scale }}</template>
                </span>
              </span>
              <i
                :class="['pi', opened === alert.id ? 'pi-chevron-up' : 'pi-chevron-down']"
                aria-hidden="true"
                class="chevron"
              />
            </button>
            <pre v-if="opened === alert.id" class="message">{{ alert.message }}</pre>
          </li>
        </ul>

        <Paginator
          v-if="filtered.length > PER_PAGE"
          :first="(page - 1) * PER_PAGE"
          :page-link-size="pageLinks"
          :rows="PER_PAGE"
          :total-records="filtered.length"
          @page="onPage"
        />
      </section>

      <p class="muted source">
        Measurements and forecasts from
        <a :href="SWPC_URL" data-ours rel="noopener" target="_blank"
          >NOAA Space Weather Prediction Center</a
        >.
      </p>
    </template>
  </div>
</template>

<style scoped>
.weather {
  gap: 1.25rem;
}

.head {
  gap: 0.4rem;
}

h1 {
  font-size: 1.6rem;
}

.blurb {
  margin: 0;
  text-wrap: pretty;
}

.panel {
  padding: 1.1rem 1.2rem 1.25rem;
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
}

.panel h2 {
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.07em;
  font-weight: 600;
}

.heading {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.key-open {
  display: inline-grid;
  place-items: center;
  padding: 0;
  border: 0;
  background: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 0.85rem;
  line-height: 1;
}

.key-open:hover,
.key-open:focus-visible {
  color: var(--accent);
}

.empty {
  padding: 1.2rem;
  margin: 0;
  color: var(--text-muted);
  text-wrap: pretty;
}

.now {
  display: grid;
  gap: 1.25rem;
  align-items: start;
}

@media (min-width: 46rem) {
  .now {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  }
}

.reading {
  gap: 0.55rem;
}

.right {
  gap: 1rem;
}

.observed {
  gap: 0.4rem;
}

.stamp {
  text-transform: none;
  letter-spacing: 0;
  font-weight: 400;
}

.odds-list {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}

.odds {
  display: flex;
  align-items: baseline;
  gap: 0.35rem;
  font-size: 0.82rem;
}

.odds .chance {
  font-variant-numeric: tabular-nums;
  font-weight: 600;
}

.odds.leading .chance {
  color: var(--diff-removed);
}

.odds .of {
  font-size: 0.92em;
}

.levels {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.level {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 0.75rem;
  font-size: 0.88rem;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  padding-bottom: 0.3rem;
}

.level:last-child {
  border-bottom: 0;
  padding-bottom: 0;
}

.level-name {
  color: var(--text-muted);
  min-width: 0;
}

.level-value {
  flex: none;
  font-weight: 600;
}

.level-value.up {
  color: var(--diff-removed);
}

.level-value .word {
  font-weight: 400;
  font-size: 0.82em;
}

.eyebrow {
  margin: 0;
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.kp {
  display: flex;
  align-items: baseline;
  gap: 0.4rem;
  margin: 0;
  flex-wrap: wrap;
}

.figure {
  font-size: 2.4rem;
  font-weight: 650;
  line-height: 1;
  letter-spacing: -0.03em;
  font-variant-numeric: tabular-nums;
}

.unit {
  font-size: 0.9rem;
}

.verdict {
  font-size: 1rem;
  font-weight: 600;
  margin-left: 0.4rem;
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
}

.threshold {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 55.55%;
  width: 2px;
  background: color-mix(in srgb, var(--text) 45%, transparent);
}

.caption,
.note {
  margin: 0;
  font-size: 0.78rem;
  text-wrap: pretty;
}

.in-force {
  gap: 0.5rem;
}

.raised {
  display: flex;
  align-items: baseline;
  gap: 0.45rem;
  margin: 0;
  font-size: 0.9rem;
  text-wrap: pretty;
}

.raised i {
  color: var(--accent);
  font-size: 0.9em;
}

.scale,
.until {
  font-size: 0.8rem;
  white-space: nowrap;
}

.quiet {
  display: flex;
  align-items: baseline;
  gap: 0.4rem;
  margin: 0;
  font-size: 0.9rem;
  text-wrap: pretty;
}

.table-scroll {
  overflow-x: auto;
}

.scales {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.88rem;
}

.scales th,
.scales td {
  text-align: left;
  padding: 0.45rem 0.6rem 0.45rem 0;
  border-top: 1px solid var(--border);
  vertical-align: baseline;
  white-space: nowrap;
}

.scales thead th {
  border-top: none;
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  font-weight: 600;
}

.scales tbody th {
  font-weight: 500;
  border-top: 1px solid var(--border);
  padding-right: 1rem;
}

/* The name of a band doubles as the way to ask what it measures. The dot beside it is the whole
   affordance: a row nobody prods reads exactly as it did before. */
.band {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.05rem;
  padding: 0;
  border: 0;
  background: none;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.hint {
  font-size: 0.72em;
  opacity: 0.55;
  margin-left: 0.15rem;
  vertical-align: 0.05em;
}

.band:hover .hint,
.band:focus-visible .hint,
.band[aria-expanded='true'] .hint {
  opacity: 1;
  color: var(--accent);
}

.about td {
  border-top: none;
  padding: 0 0 0.55rem;
  font-size: 0.8rem;
  white-space: normal;
  text-wrap: pretty;
}

.letter {
  display: inline-block;
  min-width: 1.4rem;
  font-weight: 700;
}

.what {
  font-size: 0.78rem;
}

.cell {
  display: inline-flex;
  align-items: baseline;
  gap: 0.35rem;
  font-variant-numeric: tabular-nums;
}

.cell.up {
  color: var(--accent);
  font-weight: 650;
}

.cell.blank {
  color: var(--text-muted);
}

.word {
  font-size: 0.78rem;
}

.charts {
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 20rem), 1fr));
}

.notices-head {
  justify-content: space-between;
  gap: 0.6rem 1rem;
}

.notices-head :deep(.p-togglebutton) {
  flex: none;
}

.option {
  display: inline-flex;
  align-items: baseline;
  gap: 0.35rem;
}

.tally {
  font-size: 0.78em;
  font-variant-numeric: tabular-nums;
  opacity: 0.65;
}

.notices {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
}

.notices li {
  border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
}

.notices li:first-child {
  border-top: none;
}

.notices button {
  display: flex;
  align-items: baseline;
  gap: 0.6rem;
  width: 100%;
  padding: 0.55rem 0;
  border: 0;
  background: none;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.notices button:hover .line {
  color: var(--accent);
}

.notices i {
  font-size: 0.8rem;
  color: var(--text-muted);
  flex: none;
}

.notices li.live i {
  color: var(--accent);
}

.notices .what {
  display: flex;
  flex-direction: column;
  gap: 0.05rem;
  min-width: 0;
  flex: 1;
  font-size: 0.88rem;
}

.line {
  text-wrap: pretty;
}

.when {
  font-size: 0.76rem;
  font-variant-numeric: tabular-nums;
}

.chevron {
  margin-left: auto;
  font-size: 0.7rem;
}

.message {
  margin: 0 0 0.75rem;
  padding: 0.7rem 0.85rem;
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--text) 5%, transparent);
  font-size: 0.8rem;
  line-height: 1.5;
  white-space: pre-wrap;
  overflow-x: auto;
}

.source {
  margin: 0;
  font-size: 0.8rem;
  text-wrap: pretty;
}
</style>
