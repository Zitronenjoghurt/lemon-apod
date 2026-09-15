<script lang="ts" setup>
import { computed } from 'vue'

const props = defineProps<{
  value: number
  min: number
  max: number
  minLabel: string
  minNote: string
  maxLabel: string
  maxNote: string
}>()

const at = computed(() => {
  const span = props.max - props.min
  if (span <= 0) return 0
  return Math.min(100, Math.max(0, ((props.value - props.min) / span) * 100))
})
</script>

<template>
  <div class="range">
    <div class="track">
      <div :style="{ width: `${at}%` }" class="fill" />
      <span :style="{ left: `${at}%` }" class="pin" />
    </div>
    <p class="muted ends">
      <span>
        <strong>{{ minLabel }}</strong>
        {{ minNote }}
      </span>
      <span class="far">
        <strong>{{ maxLabel }}</strong>
        {{ maxNote }}
      </span>
    </p>
  </div>
</template>

<style scoped>
.range {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.track {
  position: relative;
  height: 0.5rem;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 10%, transparent);
}

.fill {
  height: 100%;
  border-radius: 999px 0 0 999px;
  background: var(--accent);
  transition: width var(--dur-base) var(--ease-out);
}

.pin {
  position: absolute;
  top: 50%;
  width: 0.5rem;
  height: 0.5rem;
  margin-left: -0.25rem;
  transform: translateY(-50%);
  border-radius: var(--radius-pill);
  background: var(--bg-elevated);
  border: 2px solid var(--accent);
}

.ends {
  display: flex;
  justify-content: space-between;
  gap: var(--space-3);
  margin: 0;
  font-size: var(--text-xs);
  line-height: 1.4;
  font-variant-numeric: tabular-nums;
}

.ends span {
  display: flex;
  flex-direction: column;
}

.ends strong {
  font-weight: 600;
}

.far {
  text-align: right;
}
</style>
