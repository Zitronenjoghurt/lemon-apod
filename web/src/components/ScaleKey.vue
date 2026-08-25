<script lang="ts" setup>
import { computed } from 'vue'
import type { ChartBand, ChartMark, Tone } from './SeriesChart.vue'

export interface KeyRow {
  letter: string
  label: string
  effect: string
}

const props = withDefaults(
  defineProps<{
    bands?: ChartBand[]
    marks?: ChartMark[]
    rows?: KeyRow[]
  }>(),
  { bands: () => [], marks: () => [], rows: () => [] },
)

interface Entry {
  key: string
  tone?: Tone
  kind: 'band' | 'mark' | 'letter'
  letter?: string
  label?: string
  range?: string
  effect?: string
}

const scaled = computed<Entry[]>(() =>
  [
    ...props.bands.map((band) => ({
      key: `band-${Math.max(band.from, band.to)}`,
      tone: band.tone,
      kind: 'band' as const,
      at: Math.max(band.from, band.to),
      label: band.label,
      range: band.range,
      effect: band.effect,
    })),
    ...props.marks.map((mark) => ({
      key: `mark-${mark.at}`,
      tone: mark.tone ?? ('warn' as Tone),
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

const entries = computed<Entry[]>(() => [
  ...props.rows.map((row) => ({
    key: `letter-${row.letter}`,
    kind: 'letter' as const,
    letter: row.letter,
    label: row.label,
    effect: row.effect,
  })),
  ...scaled.value,
])
</script>

<template>
  <ul class="scale-key">
    <li v-for="row in entries" :key="row.key" class="entry">
      <span v-if="row.kind === 'letter'" aria-hidden="true" class="swatch letter">
        {{ row.letter }}
      </span>
      <span v-else :class="['swatch', row.kind]" :data-tone="row.tone" aria-hidden="true" />
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

.swatch.letter {
  display: grid;
  place-items: center;
  width: 1.1rem;
  height: 1.1rem;
  margin-top: 0.1rem;
  border: 1px solid var(--border);
  border-radius: 3px;
  font-size: 0.68rem;
  font-weight: 700;
  line-height: 1;
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
  color: var(--text);
  text-transform: uppercase;
  font-size: 0.7rem;
  letter-spacing: 0.05em;
}

.label[data-tone] {
  color: hsl(var(--tone));
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
