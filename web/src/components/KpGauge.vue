<script lang="ts" setup>
import { computed } from 'vue'
import { kpPercent, kpReading } from '@/utils/weather'

const props = defineProps<{ kp: number; stamp?: string }>()

const reading = computed(() => kpReading(props.kp))
const dial = computed(() => kpPercent(props.kp))
</script>

<template>
  <div class="kp-gauge">
    <p class="reading">
      <span class="figure">{{ kp.toFixed(2) }}</span>
      <span class="unit muted">Kp</span>
      <span class="verdict">{{ reading.label }}</span>
    </p>

    <div class="track">
      <div :style="{ width: `${dial}%` }" class="fill" />
      <div class="threshold" />
    </div>

    <p class="muted caption">
      Scale of 0 to 9. Storms start at 5.<template v-if="stamp"> Measured {{ stamp }}.</template>
    </p>
  </div>
</template>

<style scoped>
.kp-gauge {
  --kp-figure: 1.6rem;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.reading {
  display: flex;
  align-items: baseline;
  gap: var(--space-1);
  margin: 0;
  flex-wrap: wrap;
}

.figure {
  font-size: var(--kp-figure);
  font-weight: 650;
  line-height: 1;
  letter-spacing: -0.03em;
  font-variant-numeric: tabular-nums;
}

.unit {
  font-size: var(--text-sm);
}

.verdict {
  font-size: var(--text-md);
  font-weight: 600;
  margin-left: var(--space-1);
  text-wrap: balance;
}

.track {
  position: relative;
  height: 0.5rem;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 10%, transparent);
  overflow: hidden;
}

.fill {
  height: 100%;
  border-radius: var(--radius-pill);
  background: var(--accent);
  transition: width 0.3s ease;
}

.threshold {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 55.55%;
  width: 2px;
  background: color-mix(in srgb, var(--text) 45%, transparent);
}

.caption {
  margin: 0;
  font-size: var(--text-xs);
  text-wrap: pretty;
}
</style>
