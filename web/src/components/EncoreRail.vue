<script lang="ts" setup>
import { nextTick, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import { api } from '@/api/client'
import type { PictureAppearances } from '@/api/types'
import { formatDate, year } from '@/utils/date'

const props = defineProps<{
  date: string
  picture: string
}>()

const emit = defineEmits<{ failed: [] }>()

const encore = ref<PictureAppearances | null>(null)
const group = ref<string>()
const stops = ref<HTMLElement>()

async function load() {
  if (group.value === props.picture) return

  encore.value = null
  group.value = props.picture

  try {
    const found = await api.picture(props.picture)
    if (group.value === props.picture) encore.value = found
  } catch {
    if (group.value === props.picture) emit('failed')
  }
}

async function centreCurrentStop() {
  await nextTick()
  const rail = stops.value
  const here = rail?.querySelector<HTMLElement>('.stop.here')
  if (!rail || !here) return

  const railBox = rail.getBoundingClientRect()
  const hereBox = here.getBoundingClientRect()
  rail.scrollLeft += hereBox.left - railBox.left - (railBox.width - hereBox.width) / 2
}

watch(() => props.picture, load, { immediate: true })
watch(encore, centreCurrentStop)
</script>

<template>
  <nav aria-label="Other days this picture ran" class="rail encore">
    <template v-if="encore && encore.items.length > 1">
      <div class="row encore-row">
        <RouterLink
          v-tooltip.bottom="`Shown ${encore.picture.appearances} times`"
          :aria-label="`Shown ${encore.picture.appearances} times`"
          :to="`/pictures/${encore.picture.id}`"
          class="lead"
        >
          <AppIcon name="replay" />
          {{ encore.picture.appearances }}
        </RouterLink>

        <ol ref="stops" class="stops">
          <li v-for="item in encore.items" :key="item.date">
            <span v-if="item.date === date" aria-current="page" class="stop here">
              <span aria-hidden="true" class="dot" />
              <span class="year">{{ year(item.date) }}</span>
            </span>
            <RouterLink v-else :title="formatDate(item.date)" :to="`/${item.date}`" class="stop">
              <span aria-hidden="true" class="dot" />
              <span class="year">{{ year(item.date) }}</span>
            </RouterLink>
          </li>
        </ol>

        <RouterLink :to="`/pictures/${encore.picture.id}`" class="all">
          What changed <AppIcon name="arrow-right" />
        </RouterLink>
      </div>
    </template>

    <span v-else class="lead waiting">
      <AppIcon name="replay" />
      Shown more than once
    </span>
  </nav>
</template>

<style scoped>
.encore-row {
  gap: var(--space-3);
  align-items: center;
  flex-wrap: nowrap;
}

.lead {
  display: inline-flex;
  align-items: center;
  flex: none;
  font-variant-numeric: tabular-nums;
  gap: var(--space-1);
  color: var(--text-muted);
  text-decoration: none;
  white-space: nowrap;
  transition: color var(--dur-fast) var(--ease-out);
}

.lead:hover {
  color: var(--text);
}

.waiting {
  opacity: 0.7;
}

.stops {
  --dot: 0.5rem;
  display: flex;
  flex: 1 1 auto;
  min-width: 0;
  list-style: none;
  margin: 0;
  padding: var(--space-0) 0;
  overflow-x: auto;
  overscroll-behavior-x: contain;
  scrollbar-width: none;
}

.stops::-webkit-scrollbar {
  display: none;
}

.stops li {
  position: relative;
  flex: 1 0 2.6rem;
  display: flex;
  justify-content: center;
}

.stops li:not(:last-child)::before {
  content: '';
  position: absolute;
  top: calc(var(--dot) / 2);
  left: 50%;
  right: -50%;
  height: 1px;
  background: var(--border);
}

.stop {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 0 var(--space-1);
  text-decoration: none;
  font-variant-numeric: tabular-nums;
  font-size: var(--text-2xs);
  line-height: 1.35;
  color: var(--text-muted);
  transition: color var(--dur-fast) var(--ease-out);
}

.dot {
  width: var(--dot);
  height: var(--dot);
  border-radius: 50%;
  background: var(--bg-elevated);
  box-shadow: 0 0 0 1px var(--border) inset;
  transition:
    background var(--dur-fast) var(--ease-out),
    box-shadow var(--dur-fast) var(--ease-out),
    transform var(--dur-fast) var(--ease-out);
}

a.stop:hover {
  color: var(--text);
}

a.stop:hover .dot {
  background: color-mix(in srgb, var(--accent) 40%, transparent);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 60%, var(--border)) inset;
  transform: scale(1.25);
}

.stop.here {
  color: var(--accent);
  font-weight: 600;
}

.stop.here .dot {
  background: var(--accent);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 26%, transparent);
}

.all {
  display: inline-flex;
  align-items: center;
  flex: none;
  gap: var(--space-1);
  white-space: nowrap;
  text-decoration: none;
  color: var(--text-muted);
}

.all:hover {
  color: var(--accent);
}

.all .icon {
  font-size: 0.7em;
}
</style>
