<script lang="ts" setup>
import { computed, ref } from 'vue'

export interface SeriesPoint {
  label: string
  value: number
  ahead?: boolean
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
    threshold?: number
    thresholdLabel?: string
  }>(),
  { kind: 'bar', zeroed: undefined, decimals: 0, height: 132, unit: '' },
)

const COLUMN = 10
const TOP = 100

const fromZero = computed(() => props.zeroed ?? props.kind === 'bar')

const values = computed(() => props.points.map((point) => point.value))
const high = computed(() => Math.max(...values.value, props.threshold ?? Number.NEGATIVE_INFINITY))
const low = computed(() => (fromZero.value ? 0 : Math.min(...values.value)))

const reach = computed(() => high.value - low.value || 1)

const floor = computed(() => (fromZero.value ? 0 : low.value - reach.value * 0.15))
const ceiling = computed(() => high.value + reach.value * 0.08)
const span = computed(() => ceiling.value - floor.value || 1)

const width = computed(() => Math.max(props.points.length, 1) * COLUMN)

function y(value: number): number {
  return TOP - ((value - floor.value) / span.value) * TOP
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

const path = computed(() =>
  props.points
    .map(
      (point, index) =>
        `${index === 0 ? 'M' : 'L'}${index * COLUMN + COLUMN / 2} ${y(point.value)}`,
    )
    .join(' '),
)

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
  return point ? { x: at.value * COLUMN + COLUMN / 2, y: y(point.value) } : undefined
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
        <template v-else>{{ format(low) }} to {{ format(high) }}</template>
      </span>
    </figcaption>

    <svg
      :aria-label="`${label}, ${ends.first} to ${ends.last}`"
      :style="{ height: `${height}px` }"
      :viewBox="`0 0 ${width} ${TOP}`"
      preserveAspectRatio="none"
      role="img"
      @pointercancel="at = undefined"
      @pointerdown="read"
      @pointerleave="at = undefined"
      @pointermove="read"
      @pointerup="at = undefined"
    >
      <line
        v-if="threshold !== undefined"
        :x2="width"
        :y1="y(threshold)"
        :y2="y(threshold)"
        class="threshold"
        vector-effect="non-scaling-stroke"
        x1="0"
      />

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

    <div class="row axis muted">
      <span>{{ ends.first }}</span>
      <span v-if="thresholdLabel" class="key">{{ thresholdLabel }}</span>
      <span>{{ ends.last }}</span>
    </div>
  </figure>
</template>

<style scoped>
.series-chart {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.head {
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.85rem;
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
  font-size: 0.78rem;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.range.live {
  color: var(--text);
  font-weight: 600;
}

svg {
  width: 100%;
  display: block;
  overflow: visible;
  touch-action: pan-y;
}

.bar {
  fill: color-mix(in srgb, var(--accent) 62%, transparent);
}

.bar.ahead {
  fill: color-mix(in srgb, var(--accent) 16%, transparent);
  stroke: color-mix(in srgb, var(--accent) 60%, transparent);
  stroke-width: 1;
  stroke-dasharray: 2 2;
  vector-effect: non-scaling-stroke;
}

.bar.on {
  fill: var(--accent);
}

.line {
  stroke: var(--accent);
  stroke-width: 2;
  stroke-linejoin: round;
  stroke-linecap: round;
}

.threshold {
  stroke: color-mix(in srgb, var(--text) 35%, transparent);
  stroke-width: 1;
  stroke-dasharray: 4 4;
}

.guide {
  stroke: color-mix(in srgb, var(--text) 45%, transparent);
  stroke-width: 1;
  stroke-dasharray: 3 3;
  pointer-events: none;
}

.axis {
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
}

.key {
  font-variant-numeric: normal;
  text-align: center;
}
</style>
