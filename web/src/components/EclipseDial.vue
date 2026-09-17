<script lang="ts" setup>
import { computed, useId } from 'vue'

const props = withDefaults(
  defineProps<{
    solar: boolean
    magnitude: number
    label?: string
    size?: number | string
  }>(),
  { label: '', size: '1em' },
)

const R = 8
const CENTER = 12
const UMBRA = 21
const ANNULUS = 0.8

const id = useId()
const covered = computed(() => Math.min(Math.max(props.magnitude, 0), 1))
const annular = computed(() => props.solar && /annular/i.test(props.label))
const penumbral = computed(() => !props.solar && /penumbral/i.test(props.label))

const shadow = computed(() => {
  if (annular.value) return { cx: CENTER, r: R * ANNULUS }
  if (props.solar) return { cx: CENTER + 2 * R * (1 - covered.value), r: R }
  return { cx: CENTER + R - 2 * R * covered.value + UMBRA, r: UMBRA }
})

const percent = computed(() => Math.round(covered.value * 100))
const said = computed(() =>
  props.solar
    ? `${percent.value}% of the sun's width covered`
    : penumbral.value
      ? 'The moon in the lighter, outer part of the shadow'
      : `${percent.value}% of the moon's width in shadow`,
)
</script>

<template>
  <svg
    :aria-label="said"
    :height="size"
    :width="size"
    :class="['eclipse', { solar }]"
    role="img"
    viewBox="0 0 24 24"
  >
    <defs>
      <clipPath :id="`disc-${id}`">
        <circle :cx="CENTER" :cy="CENTER" :r="R" />
      </clipPath>
    </defs>

    <circle :cx="CENTER" :cy="CENTER" :r="R" class="body" />
    <circle
      :class="{ penumbral }"
      :clip-path="`url(#disc-${id})`"
      :cx="shadow.cx"
      :cy="CENTER"
      :r="shadow.r"
      class="shade"
    />
    <circle :cx="CENTER" :cy="CENTER" :r="R" class="rim" />
  </svg>
</template>

<style scoped>
.eclipse {
  --lit: #f4ead2;
  --edge: #262b3e;
  flex: none;
  display: inline-block;
  vertical-align: -0.145em;
  overflow: visible;
}

.eclipse.solar {
  --lit: #f5c453;
  --edge: #b8862a;
}

.body {
  fill: var(--lit);
}

.shade {
  fill: var(--blood-moon);
  transition:
    cx var(--dur-reveal) var(--ease-out),
    r var(--dur-reveal) var(--ease-out);
}

.solar .shade {
  fill: #262b3e;
}

.shade.penumbral {
  fill: #262b3e;
  fill-opacity: 0.55;
}

.rim {
  fill: none;
  stroke: color-mix(in srgb, var(--edge) 70%, transparent);
  stroke-width: 1.5;
}
</style>
