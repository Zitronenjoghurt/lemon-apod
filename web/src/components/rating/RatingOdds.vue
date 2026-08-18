<script lang="ts" setup>
import { computed } from 'vue'

const props = defineProps<{
  score: number
  lower: number
  upper: number
  comparisons: number
}>()

function odds(logit: number): number {
  return 100 / (1 + Math.exp(-logit))
}

const middle = computed(() => odds(props.score))
const from = computed(() => odds(props.lower))
const to = computed(() => odds(props.upper))

const percent = (value: number) => `${value.toFixed(value < 10 || value > 99.5 ? 1 : 0)}%`

const exact = computed(
  () =>
    `${props.score > 0 ? '+' : ''}${props.score.toFixed(2)} log-odds, ` +
    `${props.lower > 0 ? '+' : ''}${props.lower.toFixed(2)} to ` +
    `${props.upper > 0 ? '+' : ''}${props.upper.toFixed(2)}`,
)
</script>

<template>
  <div v-tooltip.top="exact" class="odds">
    <p class="figure">
      <strong>{{ percent(middle) }}</strong>
      <span class="muted range">{{ percent(from) }} to {{ percent(to) }}</span>
    </p>

    <div
      :aria-label="`Picked ${percent(middle)} of the time, somewhere between ${percent(
        from,
      )} and ${percent(to)}`"
      :aria-valuemax="100"
      :aria-valuemin="0"
      :aria-valuenow="Math.round(middle)"
      class="track"
      role="img"
    >
      <div :style="{ left: `${from}%`, width: `${Math.max(to - from, 0.6)}%` }" class="band" />
      <div :style="{ left: `${middle}%` }" class="point" />
      <div class="even" />
    </div>

    <p class="muted note">from {{ comparisons }} vote{{ comparisons === 1 ? '' : 's' }}</p>
  </div>
</template>

<style scoped>
.odds {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  min-width: 0;
}

.figure {
  display: flex;
  align-items: baseline;
  gap: 0.4rem;
  margin: 0;
  font-variant-numeric: tabular-nums;
}

.figure strong {
  font-size: 1.05rem;
  font-weight: 650;
  color: var(--text);
}

.range {
  font-size: 0.74rem;
}

.track {
  position: relative;
  height: 0.4rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  overflow: hidden;
}

.band {
  position: absolute;
  top: 0;
  bottom: 0;
  border-radius: inherit;
  background: color-mix(in srgb, var(--accent) 40%, transparent);
}

.point {
  position: absolute;
  top: -1px;
  bottom: -1px;
  width: 2px;
  margin-left: -1px;
  border-radius: 1px;
  background: var(--accent);
}

.even {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 50%;
  width: 1px;
  background: color-mix(in srgb, var(--text) 32%, transparent);
}

.note {
  margin: 0;
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
}
</style>
