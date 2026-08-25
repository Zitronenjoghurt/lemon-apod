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
  gap: 0.35rem;
}

.reading {
  display: flex;
  align-items: baseline;
  gap: 0.35rem;
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
  font-size: 0.9rem;
}

.verdict {
  font-size: 0.95rem;
  font-weight: 600;
  margin-left: 0.3rem;
  text-wrap: balance;
}

.track {
  position: relative;
  height: 0.5rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 10%, transparent);
  overflow: hidden;
}

.fill {
  height: 100%;
  border-radius: 999px;
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
  font-size: 0.75rem;
  text-wrap: pretty;
}
</style>
