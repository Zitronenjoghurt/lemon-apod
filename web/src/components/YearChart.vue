<script lang="ts" setup>
import { computed, ref } from 'vue'

const props = withDefaults(
  defineProps<{
    points: { year: number; value: number }[]
    label: string
    kind?: 'bar' | 'line'
    zeroed?: boolean
    decimals?: number
    height?: number
  }>(),
  { kind: 'bar', zeroed: undefined, decimals: 0, height: 116 },
)

const COLUMN = 10
const TOP = 100

const fromZero = computed(() => props.zeroed ?? props.kind === 'bar')

const values = computed(() => props.points.map((point) => point.value))
const high = computed(() => Math.max(...values.value, 0))
const low = computed(() => (fromZero.value ? 0 : Math.min(...values.value)))

const reach = computed(() => high.value - low.value || 1)

const floor = computed(() => (fromZero.value ? 0 : low.value - reach.value * 0.15))
const ceiling = computed(() => high.value + (fromZero.value ? 0 : reach.value * 0.15))
const span = computed(() => ceiling.value - floor.value || 1)

const width = computed(() => Math.max(props.points.length, 1) * COLUMN)

function y(value: number): number {
  return TOP - ((value - floor.value) / span.value) * TOP
}

const bars = computed(() =>
  props.points.map((point, index) => {
    const top = y(point.value)
    return {
      ...point,
      x: index * COLUMN + 1,
      y: Math.min(top, TOP - 0.5),
      height: Math.max(TOP - top, 0.5),
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
  return value.toLocaleString(undefined, {
    minimumFractionDigits: props.decimals,
    maximumFractionDigits: props.decimals,
  })
}

const years = computed(() => ({
  first: props.points[0]?.year,
  last: props.points.at(-1)?.year,
}))

const at = ref<number>()

function read(event: PointerEvent) {
  const width = (event.currentTarget as SVGElement).getBoundingClientRect().width
  if (!width || !props.points.length) return

  const share = event.offsetX / width
  at.value = Math.min(props.points.length - 1, Math.max(0, Math.floor(share * props.points.length)))
}

const reading = computed(() => (at.value === undefined ? undefined : props.points[at.value]))

const marker = computed(() => {
  if (at.value === undefined) return undefined
  const point = props.points[at.value]
  if (!point) return undefined
  return { x: at.value * COLUMN + COLUMN / 2, y: y(point.value) }
})
</script>

<template>
  <figure class="year-chart">
    <figcaption class="row head">
      <span class="name">{{ label }}</span>
      <span :class="{ live: reading }" class="muted range">
        <template v-if="reading">{{ reading.year }}: {{ format(reading.value) }}</template>
        <template v-else>{{ format(low) }} to {{ format(high) }}</template>
      </span>
    </figcaption>

    <svg
      :aria-label="`${label}, ${years.first} to ${years.last}`"
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
          :key="bar.year"
          :class="{ on: at === index }"
          :height="bar.height"
          :width="COLUMN - 2"
          :x="bar.x"
          :y="bar.y"
          class="bar"
        />
      </g>

      <template v-if="marker">
        <line
          :x1="marker.x"
          :x2="marker.x"
          class="guide"
          vector-effect="non-scaling-stroke"
          y1="0"
          y2="100"
        />
        <line
          :x2="width"
          :y1="marker.y"
          :y2="marker.y"
          class="guide"
          vector-effect="non-scaling-stroke"
          x1="0"
        />
      </template>
    </svg>

    <div class="row axis muted">
      <span>{{ years.first }}</span>
      <span>{{ years.last }}</span>
    </div>
  </figure>
</template>

<style scoped>
.year-chart {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.head {
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.85rem;
}

.name {
  font-weight: 600;
}

.range {
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

.bar.on {
  fill: var(--accent);
}

.line {
  stroke: var(--accent);
  stroke-width: 2;
  stroke-linejoin: round;
  stroke-linecap: round;
}

.guide {
  stroke: color-mix(in srgb, var(--text) 45%, transparent);
  stroke-width: 1;
  stroke-dasharray: 3 3;
  pointer-events: none;
}

.axis {
  justify-content: space-between;
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
}
</style>
