<script lang="ts" setup>
import { computed, onBeforeUnmount, ref, useId, watch } from 'vue'
import type { Hemisphere } from '@/composables/usePreferences'

const props = withDefaults(
  defineProps<{
    illumination: number
    waxing: boolean
    label: string
    size?: number
    hemisphere?: Hemisphere
  }>(),
  { size: 92, hemisphere: 'north' },
)

const R = 50
const VIEW = 120
const SWEEP_MS = 700
const DETAILED_FROM = 40
const SOFT = 6
const STEPS = 10

const BRIGHT = { r: 240, g: 238, b: 232 }
const DUSK = { r: 160, g: 155, b: 145 }

type Point = { x: number; y: number }
type Ray = { name: string; lat: number; lon: number; r: number }

const MARIA: number[][] = [
  [
    49, -12, 48, -18, 47, -22, 48, -27, 48, -32, 46, -36, 42, -37, 38, -40, 33, -40, 26, -38, 20,
    -33, 15, -28, 14, -18, 16, -8, 21, -3, 27, 1, 33, 5, 39, 3, 44, -1, 47, -6,
  ],
  [
    38, -40, 43, -38, 48, -40, 52, -44, 54, -52, 50, -60, 45, -60, 38, -62, 31, -66, 24, -70, 15,
    -70, 7, -68, 0, -64, -3, -58, -8, -52, -12, -44, -11, -36, -10, -33, -15, -30, -19, -31, -23,
    -29, -28, -24, -30, -16, -28, -10, -24, -5, -18, -3, -13, -5, -11, -10, -9, -18, -5, -20, 0,
    -18, 4, -16, 8, -13, 12, -13, 14, -20, 18, -28, 22, -35, 30, -40,
  ],
  [
    60, -40, 63, -25, 63, -10, 62, 5, 60, 20, 57, 32, 53, 40, 50, 38, 52, 30, 55, 20, 56, 10, 55, 0,
    53, -8, 53, -15, 51, -25, 52, -35, 55, -42,
  ],
  [38, 15, 37, 21, 33, 27, 28, 29, 23, 28, 19, 24, 18, 18, 20, 11, 24, 8, 30, 8, 35, 10],
  [
    18, 22, 17, 30, 16, 37, 13, 43, 8, 45, 3, 43, -2, 41, -4, 36, -5, 30, -2, 26, 2, 22, 7, 20, 12,
    20,
  ],
  [23, 58, 22, 63, 18, 66, 14, 65, 12, 60, 12.5, 54, 16, 51, 21, 53],
  [5, 46, 3, 52, -2, 57, -8, 59, -14, 58, -20, 55, -24, 50, -19, 45, -13, 43, -6, 43, -1, 44],
  [-9, 33, -11, 38, -16, 40, -20, 37, -21, 32, -17, 29, -12, 29],
  [-17, -39, -19, -34, -24, -33, -29, -35, -31, -40, -28, -45, -22, -45, -18, -43],
]

const SPOTS: [number, number, number][] = [
  [13.3, 3.6, 4],
  [12.1, -8.3, 4.5],
  [51.6, -9.3, 1.7],
  [-5.5, -68.3, 3.5],
  [40, 30, 5],
]

const RAYS: Ray[] = [
  { name: 'tycho', lat: -43.3, lon: -11.4, r: 17 },
  { name: 'copernicus', lat: 9.6, lon: -20.1, r: 9 },
  { name: 'kepler', lat: 8.1, lon: -38, r: 5.5 },
  { name: 'aristarchus', lat: 23.7, lon: -47.4, r: 5 },
  { name: 'proclus', lat: 16.1, lon: 46.8, r: 4 },
]

function place(lat: number, lon: number): Point {
  const la = (lat * Math.PI) / 180
  const lo = (lon * Math.PI) / 180
  return { x: R * Math.cos(la) * Math.sin(lo), y: -R * Math.sin(la) }
}

function ring(lat: number, lon: number, radius: number): Point[] {
  const stretch = radius / Math.cos((lat * Math.PI) / 180)
  return Array.from({ length: 10 }, (_, i) => {
    const t = (i / 10) * 2 * Math.PI
    const wobble = radius > 2 ? 1 + 0.22 * Math.sin(3 * t + lat) + 0.12 * Math.cos(5 * t) : 1
    return place(lat + radius * wobble * Math.sin(t), lon + stretch * wobble * Math.cos(t))
  })
}

function blob(points: Point[]): string {
  const n = points.length
  const at = (i: number) => points[((i % n) + n) % n]
  const fmt = (p: Point) => `${p.x.toFixed(2)},${p.y.toFixed(2)}`
  let d = `M ${fmt(at(0))}`
  for (let i = 0; i < n; i++) {
    const p0 = at(i - 1)
    const p1 = at(i)
    const p2 = at(i + 1)
    const p3 = at(i + 2)
    const c1 = { x: p1.x + (p2.x - p0.x) / 6, y: p1.y + (p2.y - p0.y) / 6 }
    const c2 = { x: p2.x - (p3.x - p1.x) / 6, y: p2.y - (p3.y - p1.y) / 6 }
    d += ` C ${fmt(c1)} ${fmt(c2)} ${fmt(p2)}`
  }
  return `${d} Z`
}

function trace(outline: number[]): Point[] {
  return Array.from({ length: outline.length / 2 }, (_, i) =>
    place(outline[2 * i], outline[2 * i + 1]),
  )
}

const MARIA_PATHS = [
  ...MARIA.map((outline) => blob(trace(outline))),
  ...SPOTS.map(([lat, lon, radius]) => blob(ring(lat, lon, radius))),
]

function cover(band: number): number {
  if (band >= STEPS - 1) return 1
  const t = (band + 1) / STEPS
  return t * t * (3 - 2 * t)
}

const HAZE = Array.from({ length: STEPS }, (_, band) => ({
  at: band / (STEPS - 1),
  opacity: band === 0 ? cover(0) : 1 - (1 - cover(band)) / (1 - cover(band - 1)),
}))

function terminator(x: number): string {
  const rx = Math.abs(x).toFixed(3)
  const sweep = x < 0 ? 0 : 1
  const out = VIEW / 2

  return `M 0,${-R} A ${rx},${R} 0 0 ${sweep} 0,${R} L 0,${out} L ${out},${out} L ${out},${-out} L 0,${-out} Z`
}

const id = useId()
const detailed = computed(() => props.size >= DETAILED_FROM)

function phaseOf(illumination: number, waxing: boolean): number {
  const lit = Math.min(Math.max(illumination, 0), 1)
  const half = Math.acos(1 - 2 * lit) / (2 * Math.PI)
  return waxing ? half : 1 - half
}

function wrap(phase: number): number {
  return ((phase % 1) + 1) % 1
}

const still = window.matchMedia('(prefers-reduced-motion: reduce)')

const shown = ref(phaseOf(props.illumination, props.waxing))
let frame: number | undefined

function sweepTo(target: number) {
  if (frame !== undefined) cancelAnimationFrame(frame)

  if (still.matches || document.hidden) {
    shown.value = target
    return
  }

  const from = shown.value
  const around = target - from
  const delta = around - Math.round(around)
  const started = performance.now()

  const step = (now: number) => {
    const t = Math.min(1, (now - started) / SWEEP_MS)
    const eased = t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2
    shown.value = wrap(from + delta * eased)
    if (t < 1) frame = requestAnimationFrame(step)
    else frame = undefined
  }

  frame = requestAnimationFrame(step)
}

watch(
  () => phaseOf(props.illumination, props.waxing),
  (target) => sweepTo(target),
)

onBeforeUnmount(() => {
  if (frame !== undefined) cancelAnimationFrame(frame)
})

const lit = computed(() => (1 - Math.cos(2 * Math.PI * shown.value)) / 2)
const waxingNow = computed(() => shown.value < 0.5)
const litOnTheRight = computed(() => waxingNow.value !== (props.hemisphere === 'south'))
const flip = computed(() => (litOnTheRight.value ? '' : 'scale(-1 1)'))
const orient = computed(() => (props.hemisphere === 'south' ? 'rotate(180)' : ''))

const edge = computed(() => R * (1 - 2 * lit.value))
const path = computed(() => terminator(edge.value))

const haze = computed(() => {
  const spread = Math.min(SOFT, R - edge.value)
  return HAZE.map((band) => ({
    d: terminator(edge.value - spread / 2 + spread * band.at),
    opacity: band.opacity,
  }))
})

const sun = computed(() => {
  const toward = 2 * lit.value - 1
  const grazing = Math.sqrt(Math.max(0, 1 - toward * toward))
  return { x: litOnTheRight.value ? grazing : -grazing, z: toward, grazing }
})

function tone(t: number): string {
  const at = (a: number, b: number) => Math.round(a + (b - a) * t)
  return `rgb(${at(BRIGHT.r, DUSK.r)} ${at(BRIGHT.g, DUSK.g)} ${at(BRIGHT.b, DUSK.b)})`
}

const shade = computed(() => ({
  from: edge.value.toFixed(3),
  dusk: tone(0.75 * sun.value.grazing),
  bright: tone(0),
}))

const glows = computed(() => {
  const side = props.hemisphere === 'south' ? -1 : 1
  const { x: sx, z: sz } = sun.value

  return RAYS.map((ray) => {
    const spot = place(ray.lat, ray.lon)
    const cx = spot.x * side
    const cy = spot.y * side
    const z = Math.sqrt(Math.max(0, R * R - cx * cx - cy * cy))
    const facing = (cx * sx + z * sz) / R
    return { name: ray.name, cx, cy, r: ray.r, opacity: 0.7 * Math.max(facing, 0) }
  })
})

const percent = computed(() => Math.round(Math.min(Math.max(props.illumination, 0), 1) * 100))
</script>

<template>
  <svg
    :aria-label="`${label}, ${percent}% lit`"
    :height="size"
    :viewBox="`${-VIEW / 2} ${-VIEW / 2} ${VIEW} ${VIEW}`"
    :width="size"
    class="moon"
    role="img"
  >
    <defs>
      <clipPath :id="`disc-${id}`">
        <circle :r="R" cx="0" cy="0" />
      </clipPath>
      <linearGradient
        :id="`lit-${id}`"
        :x1="shade.from"
        :x2="R"
        gradientUnits="userSpaceOnUse"
        y1="0"
        y2="0"
      >
        <stop :stop-color="shade.dusk" offset="0%" />
        <stop :stop-color="shade.bright" offset="100%" />
      </linearGradient>
      <radialGradient :id="`limb-${id}`" cx="50%" cy="50%" r="50%">
        <stop offset="86%" stop-color="#262b3e" stop-opacity="0" />
        <stop offset="100%" stop-color="#262b3e" stop-opacity="0.16" />
      </radialGradient>
      <template v-if="detailed">
        <radialGradient :id="`rays-${id}`">
          <stop offset="0%" stop-color="#ffffff" stop-opacity="0.9" />
          <stop offset="40%" stop-color="#ffffff" stop-opacity="0.35" />
          <stop offset="100%" stop-color="#ffffff" stop-opacity="0" />
        </radialGradient>
        <mask
          :id="`day-${id}`"
          :height="VIEW"
          :width="VIEW"
          :x="-VIEW / 2"
          :y="-VIEW / 2"
          maskUnits="userSpaceOnUse"
        >
          <g :clip-path="`url(#disc-${id})`" :transform="flip">
            <path
              v-for="(band, index) in haze"
              :key="index"
              :d="band.d"
              :opacity="band.opacity"
              fill="#fff"
            />
          </g>
        </mask>
      </template>
    </defs>

    <circle :r="R" class="dark" cx="0" cy="0" />

    <g v-if="detailed" :transform="orient" class="night">
      <path v-for="(mare, index) in MARIA_PATHS" :key="index" :d="mare" />
    </g>

    <g :clip-path="`url(#disc-${id})`">
      <g :transform="flip">
        <template v-if="detailed">
          <path
            v-for="(band, index) in haze"
            :key="index"
            :d="band.d"
            :fill="`url(#lit-${id})`"
            :opacity="band.opacity"
          />
        </template>
        <path v-else :d="path" :fill="`url(#lit-${id})`" />
      </g>
    </g>

    <g v-if="detailed" :mask="`url(#day-${id})`" class="day">
      <g :transform="orient" class="maria">
        <path v-for="(mare, index) in MARIA_PATHS" :key="index" :d="mare" />
      </g>
      <circle
        v-for="glow in glows"
        :key="glow.name"
        :cx="glow.cx"
        :cy="glow.cy"
        :fill="`url(#rays-${id})`"
        :opacity="glow.opacity"
        :r="glow.r"
      />
    </g>

    <circle :fill="`url(#limb-${id})`" :r="R" cx="0" cy="0" />
    <circle :r="R" class="rim" cx="0" cy="0" fill="none" />
  </svg>
</template>

<style scoped>
.moon {
  flex: none;
  display: block;
}

.dark {
  fill: #262b3e;
}

.night {
  fill: #0b0e1a;
  opacity: 0.3;
}

.maria {
  fill: #34405f;
  opacity: 0.17;
}

.rim {
  stroke: color-mix(in srgb, var(--text) 40%, transparent);
  stroke-width: 1.5;
}
</style>
