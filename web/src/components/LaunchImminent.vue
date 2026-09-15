<script lang="ts" setup>
import { computed, onUnmounted, ref } from 'vue'
import { RouterLink } from 'vue-router'
import type { Launch } from '@/api/types'
import { clockOf, countdown, statusTone, summaryOf, tminus } from '@/utils/launches'

const props = defineProps<{ launch: Launch }>()

const now = ref(Date.now())
const ticking = setInterval(() => (now.value = Date.now()), 1_000)
onUnmounted(() => clearInterval(ticking))

const counting = computed(() => tminus(props.launch.net, now.value))
const tone = computed(() => statusTone(props.launch.status))
</script>

<template>
  <RouterLink :class="['imminent', tone]" :to="`/launches/${launch.id}`">
    <img
      v-if="launch.image_url"
      :alt="launch.name"
      :src="launch.image_url"
      class="shot"
      decoding="async"
    />

    <span class="body">
      <span class="clock">{{ counting ?? countdown(launch.net, now) }}</span>
      <span class="title">{{ launch.name }}</span>
      <span v-if="summaryOf(launch)" class="muted detail">{{ summaryOf(launch) }}</span>
    </span>

    <span class="tail">
      <span v-if="launch.webcast_live" class="live">
        <AppIcon name="video" />
        {{ launch.status ?? 'Live now' }}
      </span>
      <span v-else-if="launch.status" :class="['flag', tone]">{{ launch.status }}</span>
      <span class="muted at">{{ clockOf(launch) }}</span>
    </span>
  </RouterLink>
</template>

<style scoped>
.imminent {
  --tone: var(--accent);

  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-4);
  border: 1px solid color-mix(in srgb, var(--tone) 45%, var(--border));
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--tone) 8%, var(--bg-elevated));
  color: inherit;
  text-decoration: none;
  transition: border-color var(--dur-fast) var(--ease-out);
}

.imminent.bad {
  --tone: var(--bad);
}

.imminent.warn {
  --tone: hsl(var(--tone-raised));
}

.imminent:hover,
.imminent:focus-visible {
  border-color: var(--tone);
}

.shot {
  width: 5.5rem;
  height: 3.5rem;
  object-fit: cover;
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
}

.body {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.clock {
  font-size: var(--text-lg);
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.02em;
}

.title {
  font-size: var(--text-sm);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.detail,
.at {
  font-size: var(--text-2xs);
}

.tail {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: var(--space-1);
  flex: none;
}

.flag,
.live {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 0 var(--space-2);
  border: 1px solid currentcolor;
  border-radius: var(--radius-pill);
  font-size: var(--text-2xs);
  white-space: nowrap;
}

.live {
  color: var(--bad);
  background: color-mix(in srgb, var(--bad) 14%, transparent);
}

.flag.bad {
  color: var(--bad);
}

.flag.warn {
  color: hsl(var(--tone-raised));
}

.flag.good {
  color: var(--good);
}

@media (max-width: 40rem) {
  .imminent {
    grid-template-columns: auto minmax(0, 1fr);
    gap: var(--space-3);
  }

  .tail {
    grid-column: 1 / -1;
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
  }
}
</style>
