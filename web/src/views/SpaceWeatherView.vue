<script lang="ts" setup>
import { computed, onMounted, ref, watch } from 'vue'
import HintPopover from '@/components/HintPopover.vue'
import KpGauge from '@/components/KpGauge.vue'
import RetryNotice from '@/components/RetryNotice.vue'
import ScaleKey from '@/components/ScaleKey.vue'
import SeriesChart, { type SeriesPoint } from '@/components/SeriesChart.vue'
import { api } from '@/api/client'
import type { NoticeKind, ScaleDay } from '@/api/types'
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
  FLUX_BANDS,
  FLUX_FRAME,
  FLUX_TICKS,
  inForce,
  KP_BANDS,
  KP_FRAME,
  KP_TICKS,
  kpReading,
  kpTone,
  KYOTO_URL,
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
const { pageLinks, roomy } = useNarrow()

const opened = ref<string>()

const kind = ref<NoticeKind | 'all'>('all')
const page = ref(1)

onMounted(run)

const now = computed(() => (report.value ? kpReading(report.value.kp) : null))
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
</script>

<template>
  <div class="stack weather">
    <header class="stack head">
      <h1>Space weather</h1>
      <p class="muted blurb">
        Solar activity and its influence on Earth, as measured and forecast by NOAA's Space Weather
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
          <p class="muted eyebrow">Planetary K index, three hour slot from {{ measuredAt }}</p>
          <KpGauge :kp="report.kp" class="big" />
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
        <h2 class="muted">
          NOAA scales forecast
          <HintPopover label="About the NOAA scales">
            <p>Three separate kinds of storms, each on its own scale of one to five.</p>
            <ScaleKey
              :rows="
                BANDS.map((band) => ({
                  letter: band.toUpperCase(),
                  label: BAND_NAMES[band],
                  effect: BAND_ABOUT[band],
                }))
              "
            />
          </HintPopover>
        </h2>

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
              <tr v-for="band in BANDS" :key="band">
                <th scope="row">
                  <span class="band">
                    <span class="letter">{{ band.toUpperCase() }}</span>
                    <span class="muted what">{{ BAND_NAMES[band] }}</span>
                  </span>
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
            </tbody>
          </table>
        </div>
      </section>

      <div class="charts">
        <section v-if="kpPoints.length" class="card panel">
          <h2 class="muted">
            Geomagnetic activity
            <HintPopover label="About geomagnetic activity">
              <p>
                Three hours per bar. Solid bars are measured, outlined bars are NOAA's
                forecast<template v-if="forecastFrom"> from {{ stamp(forecastFrom) }}</template
                >.
              </p>
              <ScaleKey :bands="KP_BANDS" />
            </HintPopover>
          </h2>
          <SeriesChart
            :bands="KP_BANDS"
            :decimals="2"
            :frame="KP_FRAME"
            :points="kpPoints"
            :ticks="KP_TICKS"
            label="Kp, measured and forecast"
          />
        </section>

        <section v-if="fluxPoints.length" class="card panel">
          <h2 class="muted">
            Solar radio flux
            <HintPopover label="About the solar radio flux">
              <p>
                The sun's radio output at a wavelength of 10.7 cm, in solar flux units. It tracks
                the active regions on the disc and therefore shows how active the sun is.
              </p>
              <ScaleKey :bands="FLUX_BANDS" />
            </HintPopover>
          </h2>
          <SeriesChart
            :bands="FLUX_BANDS"
            :frame="FLUX_FRAME"
            :points="fluxPoints"
            :ticks="FLUX_TICKS"
            :zeroed="false"
            kind="line"
            label="F10.7, thirty days"
            unit=" sfu"
          />
        </section>

        <section v-if="dstPoints.length" class="card panel">
          <h2 class="muted">
            Ring current
            <HintPopover label="About the ring current">
              <p>
                How far the ring current circling Earth has weakened the magnetic field at the
                equator. A storm drives it down as that current builds, and an arriving shock
                briefly pushes it the other way.
              </p>
              <ScaleKey :bands="DST_BANDS" :marks="DST_MARKS" />
            </HintPopover>
          </h2>
          <SeriesChart
            :bands="DST_BANDS"
            :frame="DST_FRAME"
            :mark-labels="false"
            :marks="DST_MARKS"
            :points="dstPoints"
            :ticks="DST_TICKS"
            :zeroed="false"
            kind="line"
            label="Dst, the last week"
            unit=" nT"
          />
        </section>
      </div>

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
        Measured and forecast by
        <a :href="SWPC_URL" data-ours rel="noopener" target="_blank"
          >NOAA's Space Weather Prediction Center</a
        >, who also
        <a :href="SCALES_URL" data-ours rel="noopener" target="_blank">explain their scales</a>. The
        ring current index is compiled by the
        <a :href="KYOTO_URL" data-ours rel="noopener" target="_blank"
          >World Data Center for Geomagnetism, Kyoto</a
        >.
      </p>
    </template>
  </div>
</template>

<style scoped>
.weather {
  gap: var(--space-5);
}

.head {
  gap: var(--space-2);
}

h1 {
  font-size: var(--text-xl);
}

.blurb {
  margin: 0;
  text-wrap: pretty;
}

.panel {
  padding: var(--space-4) var(--space-5) var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.panel h2 {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: var(--space-2);
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.07em;
  font-weight: 600;
}

.empty {
  padding: var(--space-5);
  margin: 0;
  color: var(--text-muted);
  text-wrap: pretty;
}

.now {
  display: grid;
  gap: var(--space-5);
  align-items: start;
}

@media (min-width: 46rem) {
  .now {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  }
}

.reading {
  gap: var(--space-2);
}

.big {
  --kp-figure: 2.4rem;
}

.right {
  gap: var(--space-4);
}

.observed {
  gap: var(--space-2);
}

.stamp {
  text-transform: none;
  letter-spacing: 0;
  font-weight: 400;
}

.odds-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-0);
}

.odds {
  display: flex;
  align-items: baseline;
  gap: var(--space-1);
  font-size: var(--text-sm);
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
  gap: var(--space-1);
}

.level {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-3);
  font-size: var(--text-sm);
  border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  padding-bottom: var(--space-1);
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
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.note {
  margin: 0;
  font-size: var(--text-xs);
  text-wrap: pretty;
}

.in-force {
  gap: var(--space-2);
}

.raised {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.raised i {
  color: var(--accent);
  font-size: 0.9em;
}

.scale,
.until {
  font-size: var(--text-sm);
  white-space: nowrap;
}

.quiet {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}

.table-scroll {
  overflow-x: auto;
}

.scales {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--text-sm);
}

.scales th,
.scales td {
  text-align: left;
  padding: var(--space-2) var(--space-2) var(--space-2) 0;
  border-top: 1px solid var(--border);
  vertical-align: baseline;
  white-space: nowrap;
}

.scales thead th {
  border-top: none;
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  font-weight: 600;
}

.scales tbody th {
  font-weight: 500;
  border-top: 1px solid var(--border);
  padding-right: var(--space-4);
}

.band {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-0);
}

.letter {
  font-weight: 700;
}

.what {
  font-size: var(--text-xs);
}

.cell {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-1);
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
  font-size: var(--text-xs);
}

.charts {
  display: grid;
  gap: var(--gap);
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 20rem), 1fr));
}

.notices-head {
  justify-content: space-between;
  gap: var(--space-2) var(--space-4);
}

.notices-head :deep(.p-togglebutton) {
  flex: none;
}

.option {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-1);
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
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-2) 0;
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
  font-size: var(--text-sm);
  color: var(--text-muted);
  flex: none;
}

.notices li.live i {
  color: var(--accent);
}

.notices .what {
  display: flex;
  flex-direction: column;
  gap: var(--space-0);
  min-width: 0;
  flex: 1;
  font-size: var(--text-sm);
}

.line {
  text-wrap: pretty;
}

.when {
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
}

.chevron {
  margin-left: auto;
  font-size: var(--text-xs);
}

.message {
  margin: 0 0 var(--space-3);
  padding: var(--space-3) var(--space-3);
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--text) 5%, transparent);
  font-size: var(--text-sm);
  line-height: 1.5;
  white-space: pre-wrap;
  overflow-x: auto;
}

.source {
  margin: 0;
  font-size: var(--text-sm);
  text-wrap: pretty;
}
</style>
