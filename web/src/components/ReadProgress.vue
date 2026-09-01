<script lang="ts" setup>
import { computed } from 'vue'

const props = defineProps<{
  read: number
  total?: number
  label: string
  bare?: boolean
}>()

const share = computed(() => {
  if (!props.total) return 0
  return Math.min(1, props.read / props.total)
})

const percent = computed(() => Math.round(share.value * 100))

const complete = computed(() => Boolean(props.total) && props.read >= (props.total ?? 0))
</script>

<template>
  <div v-if="total" :class="{ bare }" class="read-progress">
    <span
      :aria-label="`Read progress for ${label}`"
      :aria-valuenow="percent"
      aria-valuemax="100"
      aria-valuemin="0"
      class="meter"
      role="progressbar"
    >
      <span
        :class="{ done: complete }"
        :style="{ width: `${Math.max(share * 100, 1)}%` }"
        class="fill"
      />
    </span>

    <span v-if="bare" class="muted note tabular">{{ percent }}%</span>

    <span v-else class="muted note">
      <template v-if="complete">
        <i aria-hidden="true" class="pi pi-check" />
        All {{ total.toLocaleString() }} read in {{ label }}
      </template>
      <template v-else>
        {{ read.toLocaleString() }} of {{ total.toLocaleString() }} entries read in {{ label }}
        <span class="tabular">({{ percent }}%)</span>
      </template>
    </span>
  </div>
</template>

<style scoped>
.read-progress {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.read-progress.bare .meter {
  max-width: none;
}

.meter {
  flex: 1 1 8rem;
  min-width: 5rem;
  max-width: 18rem;
  height: 0.42rem;
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 10%, transparent);
  overflow: hidden;
}

.fill {
  display: block;
  height: 100%;
  border-radius: var(--radius-pill);
  background: var(--accent);
  transition: width 0.25s ease;
}

.fill.done {
  background: color-mix(in srgb, var(--accent) 70%, var(--good));
}

.note {
  font-size: var(--text-sm);
  white-space: nowrap;
}

.note i {
  font-size: 0.75em;
}

.tabular {
  font-variant-numeric: tabular-nums;
}
</style>
