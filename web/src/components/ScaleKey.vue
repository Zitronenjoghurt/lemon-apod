<script lang="ts" setup>
import { computed } from 'vue'
import type { ChartBand, ChartMark } from './SeriesChart.vue'

const props = withDefaults(
  defineProps<{
    bands?: ChartBand[]
    marks?: ChartMark[]
  }>(),
  { bands: () => [], marks: () => [] },
)

const rows = computed(() =>
  [
    ...props.bands.map((band) => ({
      tone: band.tone,
      kind: 'band' as const,
      at: Math.max(band.from, band.to),
      label: band.label,
      range: band.range,
      effect: band.effect,
    })),
    ...props.marks.map((mark) => ({
      tone: mark.tone ?? 'warn',
      kind: 'mark' as const,
      at: mark.at,
      label: mark.label,
      range: mark.range,
      effect: mark.effect,
    })),
  ]
    .filter((row) => row.effect)
    .sort((one, two) => two.at - one.at),
)
</script>

<template>
  <ul class="scale-key">
    <li v-for="row in rows" :key="`${row.kind}-${row.at}`" class="entry">
      <span :class="['swatch', row.kind]" :data-tone="row.tone" aria-hidden="true" />
      <span class="text">
        <span class="head">
          <span v-if="row.label" :data-tone="row.tone" class="label">{{ row.label }}</span>
          <span v-if="row.range" class="muted range">{{ row.range }}</span>
        </span>
        <span class="muted effect">{{ row.effect }}</span>
      </span>
    </li>
  </ul>
</template>

<style scoped>
.scale-key {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
  font-size: 0.85rem;
  max-width: 22rem;
}

.entry {
  display: flex;
  gap: 0.6rem;
  align-items: start;
}

.swatch {
  flex: none;
  width: 0.8rem;
  height: 0.8rem;
  margin-top: 0.28rem;
  border-radius: 2px;
}

.swatch.band {
  background: hsl(var(--tone) / 0.22);
  border: 1px solid hsl(var(--tone) / 0.6);
}

.swatch.mark {
  position: relative;
}

.swatch.mark::before {
  content: '';
  position: absolute;
  inset-inline: 0;
  top: 50%;
  border-top: 2px solid hsl(var(--tone) / 0.85);
}

.swatch[data-tone='calm'],
.label[data-tone='calm'] {
  --tone: var(--tone-calm);
}

.swatch[data-tone='raised'],
.label[data-tone='raised'] {
  --tone: var(--tone-raised);
}

.swatch[data-tone='warn'],
.label[data-tone='warn'] {
  --tone: var(--tone-warn);
}

.swatch[data-tone='alert'],
.label[data-tone='alert'] {
  --tone: var(--tone-alert);
}

.swatch[data-tone='severe'],
.label[data-tone='severe'] {
  --tone: var(--tone-severe);
}

.text {
  display: flex;
  flex-direction: column;
  gap: 0.12rem;
  min-width: 0;
}

.head {
  display: flex;
  align-items: baseline;
  gap: 0.4rem;
  flex-wrap: wrap;
}

.label {
  font-weight: 600;
  color: hsl(var(--tone));
  text-transform: uppercase;
  font-size: 0.7rem;
  letter-spacing: 0.05em;
}

.range {
  font-size: 0.75rem;
  font-variant-numeric: tabular-nums;
}

.effect {
  text-wrap: pretty;
  line-height: 1.45;
}
</style>
