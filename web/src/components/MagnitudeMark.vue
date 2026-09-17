<script lang="ts" setup>
import { computed } from 'vue'

const props = withDefaults(defineProps<{ magnitude: number; size?: number | string }>(), {
  size: '1em',
})

const BRIGHTEST = -4.7
const FAINTEST = 6

const brightness = computed(
  () => 1 - Math.min(Math.max((props.magnitude - BRIGHTEST) / (FAINTEST - BRIGHTEST), 0), 1),
)

const core = computed(() => 2.2 + 2.2 * brightness.value)
const spike = computed(() => (brightness.value > 0.35 ? 4 + 7 * (brightness.value - 0.35) : 0))
const glow = computed(() => 0.55 + 0.45 * brightness.value)

const spikes = computed(() => {
  const length = spike.value
  if (length <= 0) return ''
  const from = core.value + 1.2
  const to = from + length
  const diagonalFrom = from * 0.75
  const diagonalTo = (from + length * 0.5) * 0.7071
  return [
    `M12 ${12 - from}V${12 - to}`,
    `M12 ${12 + from}V${12 + to}`,
    `M${12 - from} 12H${12 - to}`,
    `M${12 + from} 12H${12 + to}`,
    `M${12 - diagonalFrom} ${12 - diagonalFrom}L${12 - diagonalTo} ${12 - diagonalTo}`,
    `M${12 + diagonalFrom} ${12 - diagonalFrom}L${12 + diagonalTo} ${12 - diagonalTo}`,
    `M${12 - diagonalFrom} ${12 + diagonalFrom}L${12 - diagonalTo} ${12 + diagonalTo}`,
    `M${12 + diagonalFrom} ${12 + diagonalFrom}L${12 + diagonalTo} ${12 + diagonalTo}`,
  ].join('')
})

const said = computed(
  () => `magnitude ${props.magnitude < 0 ? '−' : '+'}${Math.abs(props.magnitude).toFixed(1)}`,
)
</script>

<template>
  <svg
    :aria-label="said"
    :height="size"
    :style="{ opacity: glow }"
    :width="size"
    class="magnitude"
    role="img"
    viewBox="0 0 24 24"
  >
    <path v-if="spikes" :d="spikes" class="spikes" />
    <circle :r="core" class="core" cx="12" cy="12" />
  </svg>
</template>

<style scoped>
.magnitude {
  flex: none;
  display: inline-block;
  vertical-align: -0.145em;
  overflow: visible;
}

.core {
  fill: currentColor;
  transition: r var(--dur-slow) var(--ease-out);
}

.spikes {
  fill: none;
  stroke: currentColor;
  stroke-width: 1.5;
  stroke-linecap: round;
}
</style>
