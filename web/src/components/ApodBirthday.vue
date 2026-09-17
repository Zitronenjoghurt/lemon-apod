<script lang="ts" setup>
import { computed } from 'vue'
import { birthdayAge } from '@/utils/date'

const props = defineProps<{
  date: string
  today?: boolean
  compact?: boolean
}>()

const age = computed(() => birthdayAge(props.date))

const wording = computed(() =>
  props.today ? `Astronomy Picture of the Day turns ${age.value}` : `APOD turned ${age.value}`,
)
</script>

<template>
  <span v-if="age" :class="['birthday', { compact }]" :title="`${wording} on this day`">
    <AppIcon class="cake" name="birthday" />
    <span v-if="compact" class="sr-only">{{ wording }} on this day</span>
    <template v-else>{{ wording }}</template>
  </span>
</template>

<style scoped>
.birthday {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  border: 1px solid color-mix(in srgb, var(--accent) 40%, var(--border));
  border-radius: var(--radius-pill);
  padding: 0 var(--space-2);
  background: color-mix(in srgb, var(--accent) 10%, transparent);
  color: var(--text);
  font-size: var(--text-xs);
  white-space: nowrap;
}

.birthday .cake {
  color: var(--accent);
  font-size: 1.2em;
}

.compact {
  padding: 0;
  border: 0;
  background: none;
}

.compact .cake {
  color: inherit;
  font-size: var(--text-sm);
}
</style>
