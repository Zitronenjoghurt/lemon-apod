<script lang="ts" setup>
import { READ_FILTERS, useRead } from '@/composables/useRead'

const { filter } = useRead()

defineProps<{
  hidden?: number
}>()
</script>

<template>
  <div class="read-filter">
    <SelectButton
      v-model="filter"
      :allow-empty="false"
      :options="READ_FILTERS"
      aria-labelledby="read-filter-label"
      option-label="label"
      option-value="value"
      size="small"
    >
      <template #option="{ option }">
        <i :class="option.icon" aria-hidden="true" />
        <span class="label">{{ option.label }}</span>
      </template>
    </SelectButton>
    <span id="read-filter-label" class="sr-only">Filter by read state</span>

    <span v-if="hidden" aria-live="polite" class="muted hidden-note">
      {{ hidden }} hidden on this page
    </span>
  </div>
</template>

<style scoped>
.read-filter {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  flex-wrap: wrap;
}

.label {
  margin-left: 0.35rem;
}

.hidden-note {
  font-size: 0.8rem;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}

@media (max-width: 26rem) {
  .label {
    display: none;
  }
}
</style>
