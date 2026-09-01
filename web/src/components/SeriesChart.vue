<script lang="ts" setup>
import { computed, ref } from 'vue'

export type Tone = 'calm' | 'raised' | 'warn' | 'alert' | 'severe'

export interface SeriesPoint {
  label: string
  value: number
  ahead?: boolean
  tone?: Tone
}

export interface ChartBand {
  from: number
  to: number
  tone: Tone
  label?: string
  range?: string
  effect?: string
}

export interface ChartMark {
  at: number
  label: string
  tone?: Tone
  range?: string
  effect?: string
}

const props = withDefaults(
  defineProps<{
    points: SeriesPoint[]
    label: string
    kind?: 'bar' | 'line'
    zeroed?: boolean
    decimals?: number
    height?: number
    unit?: string
    bands?: ChartBand[]
    marks?: ChartMark[]
    markLabels?: boolean
    frame?: { min?: number; max?: number }
    ticks?: number[]
  }>(),
  {
    kind: 'bar',
    zeroed: undefined,
    decimals: 0,
    height: 132,
    unit: '',
    bands: () => [],
    marks: () => [],
    markLabels: true,
    frame: () => ({}),
    ticks: () => [],
  },
)

const COLUMN = 10
const TOP = 100

const fromZero = computed(() => props.zeroed ?? props.kind === 'bar')

const values = computed(() => props.points.map((point) => point.value))

const seen = computed(() => ({
  low: Math.min(...values.value),
  high: Math.max(...values.value),
}))

const pinnedLow = computed(() => props.frame.min ?? (fromZero.value ? 0 : undefined))

const low = computed(() => Math.min(seen.value.low, pinnedLow.value ?? Number.POSITIVE_INFINITY))
const high = computed(() => Math.max(seen.value.high, props.frame.max ?? Number.NEGATIVE_INFINITY))

const reach = computed(() => high.value - low.value || 1)

const floor = computed(() => {
  const pinned = pinnedLow.value
  if (pinned === undefined) return seen.value.low - reach.value * 0.15
  return seen.value.low < pinned ? seen.value.low - reach.value * 0.08 : pinned
})

const ceiling = computed(() => {
  const pinned = props.frame.max
  if (pinned === undefined) return seen.value.high + reach.value * 0.08
  return seen.value.high > pinned ? seen.value.high + reach.value * 0.08 : pinned
})

const span = computed(() => ceiling.value - floor.value || 1)

const width = computed(() => Math.max(props.points.length, 1) * COLUMN)

function y(value: number): number {
  return TOP - ((value - floor.value) / span.value) * TOP
}

function down(value: number): string {
  return `${Math.min(100, Math.max(0, y(value)))}%`
}

const base = computed(() => Math.min(Math.max(y(Math.max(floor.value, 0)), 0), TOP))

const bars = computed(() =>
  props.points.map((point, index) => {
    const top = y(point.value)
    const from = Math.min(top, base.value)
    return {
      ...point,
      x: index * COLUMN + 1,
      y: from,
      height: Math.max(Math.abs(base.value - top), 0.6),
    }
  }),
)

function xAt(index: number): number {
  if (props.kind !== 'line') return index * COLUMN + COLUMN / 2

  const last = props.points.length - 1
  return last > 0 ? (index / last) * width.value : width.value / 2
}

const path = computed(() =>
  props.points
    .map((point, index) => `${index === 0 ? 'M' : 'L'}${xAt(index)} ${y(point.value)}`)
    .join(' '),
)

const LABEL_ROOM = 16

const zones = computed(() =>
  props.bands
    .map((band) => {
      const top = Math.max(Math.min(band.from, band.to), floor.value)
      const bottom = Math.min(Math.max(band.from, band.to), ceiling.value)
      const height = y(top) - y(bottom)
      return {
        ...band,
        top: y(bottom),
        height,
        roomy: (height / 100) * props.height >= LABEL_ROOM,
      }
    })
    .filter((band) => band.height > 0.5),
)

const lines = computed(() =>
  props.marks.filter((mark) => mark.at >= floor.value && mark.at <= ceiling.value),
)

const rows = computed(() =>
  props.ticks
    .filter((tick) => tick >= floor.value && tick <= ceiling.value)
    .map((tick) => {
      const top = Math.min(100, Math.max(0, y(tick)))
      return { tick, top, edge: top > 92 ? 'low' : top < 8 ? 'high' : undefined }
    }),
)

function tickLabel(value: number): string {
  return value.toLocaleString(undefined, { maximumFractionDigits: 1 })
}

function format(value: number): string {
  return `${value.toLocaleString(undefined, {
    minimumFractionDigits: props.decimals,
    maximumFractionDigits: props.decimals,
  })}${props.unit}`
}

const at = ref<number>()

function read(event: PointerEvent) {
  const box = (event.currentTarget as SVGElement).getBoundingClientRect().width
  if (!box || !props.points.length) return

  const share = event.offsetX / box
  at.value = Math.min(props.points.length - 1, Math.max(0, Math.floor(share * props.points.length)))
}

const reading = computed(() => (at.value === undefined ? undefined : props.points[at.value]))

const marker = computed(() => {
  if (at.value === undefined) return undefined
  const point = props.points[at.value]
  return point ? { x: xAt(at.value), y: y(point.value) } : undefined
})

const ends = computed(() => ({
  first: props.points[0]?.label,
  last: props.points.at(-1)?.label,
}))
</script>

<template>
  <figure class="series-chart">
    <figcaption class="row head">
      <span class="name">{{ label }}</span>
      <span :class="{ live: reading }" class="muted range">
        <template v-if="reading">{{ reading.label }}: {{ format(reading.value) }}</template>
        <template v-else>{{ format(seen.low) }} to {{ format(seen.high) }}</template>
      </span>
    </figcaption>

    <div :style="{ height: `${height}px` }" class="plot">
      <div
        v-for="(zone, index) in zones"
        :key="`zone-${index}`"
        :data-tone="zone.tone"
        :style="{ top: `${zone.top}%`, height: `${zone.height}%` }"
        class="zone"
      >
        <span v-if="zone.label && zone.roomy" class="zone-label">{{ zone.label }}</span>
      </div>

      <div
        v-for="row in rows"
        :key="`tick-${row.tick}`"
        :style="{ top: `${row.top}%` }"
        aria-hidden="true"
        class="tick"
      >
        <span :class="['tick-label', row.edge]">{{ tickLabel(row.tick) }}</span>
      </div>

      <svg
        :aria-label="`${label}, ${ends.first} to ${ends.last}`"
        :viewBox="`0 0 ${width} ${TOP}`"
        preserveAspectRatio="none"
        role="img"
        @pointercancel="at = undefined"
        @pointerdown="read"
        @pointerleave="at = undefined"
        @pointermove="read"
        @pointerup="at = undefined"
      >
        <path
          v-if="kind === 'line'"
          :d="path"
          class="line"
          fill="none"
          vector-effect="non-scaling-stroke"
        />

        <g v-else>
          <rect
            v-for="(bar, index) in bars"
            :key="index"
            :class="{ on: at === index, ahead: bar.ahead }"
            :data-tone="bar.tone"
            :height="bar.height"
            :width="COLUMN - 2"
            :x="bar.x"
            :y="bar.y"
            class="bar"
          />
        </g>

        <line
          v-if="marker"
          :x1="marker.x"
          :x2="marker.x"
          class="guide"
          vector-effect="non-scaling-stroke"
          y1="0"
          y2="100"
        />
      </svg>

      <div
        v-for="mark in lines"
        :key="mark.label"
        :data-tone="mark.tone ?? 'warn'"
        :style="{ top: down(mark.at) }"
        class="mark"
      >
        <span v-if="markLabels" class="mark-label">{{ mark.label }}</span>
      </div>
    </div>

    <div class="row axis muted">
      <span>{{ ends.first }}</span>
      <span>{{ ends.last }}</span>
    </div>
  </figure>
</template>

<style scoped>
.series-chart {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.head {
  justify-content: space-between;
  gap: var(--space-2);
  font-size: var(--text-sm);
  flex-wrap: nowrap;
}

.name {
  font-weight: 600;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.range {
  flex: none;
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.range.live {
  color: var(--text);
  font-weight: 600;
}

.plot {
  position: relative;
  isolation: isolate;
}

svg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  display: block;
  touch-action: pan-y;
  z-index: 2;
}

.zone {
  position: absolute;
  inset-inline: 0;
  z-index: 0;
  background: hsl(var(--tone) / var(--zone-fill));
  border-top: 1px solid hsl(var(--tone) / var(--zone-edge));
  animation: settle var(--dur-slow) var(--ease-out) backwards;
}

@keyframes settle {
  from {
    opacity: 0;
  }
}

.zone:first-of-type {
  border-top: 0;
}

.zone[data-tone='calm'] {
  --tone: var(--tone-calm);
}

.zone[data-tone='raised'] {
  --tone: var(--tone-raised);
}

.zone[data-tone='warn'] {
  --tone: var(--tone-warn);
}

.zone[data-tone='alert'] {
  --tone: var(--tone-alert);
}

.zone[data-tone='severe'] {
  --tone: var(--tone-severe);
}

.zone-label {
  position: absolute;
  right: 0.25rem;
  top: 0.1rem;
  font-size: var(--text-2xs);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: hsl(var(--tone) / 0.95);
  white-space: nowrap;
  pointer-events: none;
}

.tick {
  position: absolute;
  inset-inline: 0;
  height: 0;
  border-top: 1px dashed color-mix(in srgb, var(--text) 12%, transparent);
  z-index: 1;
}

.tick-label.low {
  transform: translateY(-100%);
}

.tick-label.high {
  transform: none;
}

.tick-label {
  position: absolute;
  left: 0;
  top: 0;
  transform: translateY(-50%);
  z-index: 4;
  padding-right: var(--space-1);
  font-size: var(--text-2xs);
  font-variant-numeric: tabular-nums;
  color: var(--text-muted);
  pointer-events: none;
}

.mark {
  position: absolute;
  inset-inline: 0;
  height: 0;
  border-top: 1px solid hsl(var(--tone) / 0.75);
  z-index: 3;
  pointer-events: none;
}

.mark[data-tone='calm'] {
  --tone: var(--tone-calm);
}

.mark[data-tone='raised'] {
  --tone: var(--tone-raised);
}

.mark[data-tone='warn'] {
  --tone: var(--tone-warn);
}

.mark[data-tone='alert'] {
  --tone: var(--tone-alert);
}

.mark[data-tone='severe'] {
  --tone: var(--tone-severe);
}

.mark-label {
  position: absolute;
  right: 0;
  top: 0;
  transform: translateY(-50%);
  padding: 0 var(--space-1);
  font-size: var(--text-2xs);
  font-weight: 600;
  color: hsl(var(--tone));
  background: var(--bg-elevated);
  white-space: nowrap;
}

.bar {
  fill: color-mix(in srgb, var(--accent) 62%, transparent);
  transform-box: fill-box;
  transform-origin: bottom;
  animation: grow var(--dur-reveal) var(--ease-out) backwards;
}

@keyframes grow {
  from {
    transform: scaleY(0);
  }
}

.bar[data-tone] {
  fill: hsl(var(--tone) / 0.75);
}

.bar[data-tone='calm'] {
  --tone: var(--tone-calm);
}

.bar[data-tone='raised'] {
  --tone: var(--tone-raised);
}

.bar[data-tone='warn'] {
  --tone: var(--tone-warn);
}

.bar[data-tone='alert'] {
  --tone: var(--tone-alert);
}

.bar[data-tone='severe'] {
  --tone: var(--tone-severe);
}

/* Forecast bars are drawn as outlines, so measured and predicted never read the same. */
.bar.ahead {
  fill: color-mix(in srgb, var(--accent) 16%, transparent);
  stroke: color-mix(in srgb, var(--accent) 60%, transparent);
  stroke-width: 1;
  stroke-dasharray: 2 2;
  vector-effect: non-scaling-stroke;
}

.bar.ahead[data-tone] {
  fill: hsl(var(--tone) / 0.14);
  stroke: hsl(var(--tone) / 0.7);
}

.bar.on {
  fill: var(--accent);
}

.bar.on[data-tone] {
  fill: hsl(var(--tone));
}

.line {
  stroke: var(--accent);
  stroke-width: 2;
  stroke-linejoin: round;
  stroke-linecap: round;
  animation: draw var(--dur-reveal) var(--ease-in-out) backwards;
}

@keyframes draw {
  from {
    clip-path: inset(0 100% 0 0);
  }
  to {
    clip-path: inset(0 0 0 0);
  }
}

.guide {
  stroke: color-mix(in srgb, var(--text) 45%, transparent);
  stroke-width: 1;
  stroke-dasharray: 3 3;
  pointer-events: none;
}

.axis {
  justify-content: space-between;
  gap: var(--space-2);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
}
</style>
