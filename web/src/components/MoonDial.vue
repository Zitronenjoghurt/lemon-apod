<script lang="ts" setup>
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    illumination: number
    waxing: boolean
    label: string
    size?: number
  }>(),
  { size: 92 },
)

const R = 50
const VIEW = 120

const lit = computed(() => Math.min(Math.max(props.illumination, 0), 1))
const path = computed(() => {
  const rx = (R * Math.abs(1 - 2 * lit.value)).toFixed(3)
  const bulge = lit.value > 0.5 ? 1 : 0

  return `M 0,${-R} A ${R},${R} 0 0 1 0,${R} A ${rx},${R} 0 0 ${bulge} 0,${-R} Z`
})
const flip = computed(() => (props.waxing ? '' : 'scale(-1 1)'))

const percent = computed(() => Math.round(lit.value * 100))
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
      <radialGradient :id="`glow-${percent}`" cx="38%" cy="34%" r="72%">
        <stop offset="0%" stop-color="#fffdf4" />
        <stop offset="62%" stop-color="#f4ead2" />
        <stop offset="100%" stop-color="#d9cbab" />
      </radialGradient>
    </defs>

    <circle :r="R" class="dark" cx="0" cy="0" />

    <g :transform="flip">
      <path :d="path" :fill="`url(#glow-${percent})`" />
    </g>

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

.rim {
  stroke: color-mix(in srgb, #262b3e 55%, transparent);
  stroke-width: 1.5;
}
</style>
