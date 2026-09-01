<script lang="ts" setup>
import { computed } from 'vue'
import type { RatingProgress, RatingStage } from '@/api/types'

const props = withDefaults(
  defineProps<{
    progress: RatingProgress
    bare?: boolean
    label?: string
  }>(),
  { bare: false, label: '' },
)

const STAGES: Record<RatingStage, { name: string; buys: string }> = {
  screen: {
    name: 'Screening',
    buys: 'currently sorting the whole archive into coarse tiers',
  },
  contend: {
    name: 'Narrowing',
    buys: 'currently separating the uppermost tier to narrow down the best',
  },
  settle: {
    name: 'Settling',
    buys: 'currently trying to separate topmost picks',
  },
  settled: {
    name: 'Settled',
    buys: 'the best picture has been found',
  },
}

const stage = computed(() => STAGES[props.progress.stage])

const percent = computed(() => {
  const { done, target } = props.progress
  if (target <= 0) return 100
  return Math.min(100, Math.round((done / target) * 1000) / 10)
})

const counted = computed(() => props.progress.done.toLocaleString())
const wanted = computed(() => props.progress.target.toLocaleString())
</script>

<template>
  <div class="rating-progress">
    <p v-if="!bare" class="head">
      <span class="stage">{{ label || stage.name }}</span>
      <span class="muted count">{{ counted }} of {{ wanted }} votes</span>
    </p>

    <div
      :aria-label="`${stage.name}: ${counted} of ${wanted} votes`"
      :aria-valuemax="progress.target"
      :aria-valuenow="progress.done"
      aria-valuemin="0"
      class="track"
      role="progressbar"
    >
      <div :style="{ width: `${percent}%` }" class="filled" />
    </div>

    <p v-if="!bare" class="muted buys">{{ stage.buys }}</p>
  </div>
</template>

<style scoped>
.rating-progress {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-sm);
  flex-wrap: wrap;
}

.stage {
  font-weight: 600;
}

.count {
  font-size: var(--text-sm);
  font-variant-numeric: tabular-nums;
}

.track {
  height: 0.45rem;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 10%, transparent);
  overflow: hidden;
}

.filled {
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
  transition: width 0.4s ease;
}

.buys {
  margin: 0;
  font-size: var(--text-xs);
  text-wrap: pretty;
}
</style>
