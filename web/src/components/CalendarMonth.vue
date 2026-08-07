<script lang="ts" setup>
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import type { ApodSummary } from '@/api/types'
import { useRead } from '@/composables/useRead'

const props = defineProps<{
  year: number
  month: number
  entries: ApodSummary[]
  loading?: boolean
}>()

const { isRead, dimmed } = useRead()

const WEEKDAYS = Array.from({ length: 7 }, (_, index) =>
  new Date(Date.UTC(2024, 0, index + 1)).toLocaleDateString(undefined, { weekday: 'short' }),
)

const byDate = computed(() => new Map(props.entries.map((entry) => [entry.date, entry])))

const padded = computed(() => String(props.month).padStart(2, '0'))

const lead = computed(
  () => (new Date(Date.UTC(props.year, props.month - 1, 1)).getUTCDay() + 6) % 7,
)

const days = computed(() => {
  const count = new Date(Date.UTC(props.year, props.month, 0)).getUTCDate()

  return Array.from({ length: count }, (_, index) => {
    const day = index + 1
    const date = `${props.year}-${padded.value}-${String(day).padStart(2, '0')}`
    return { day, date, entry: byDate.value.get(date) }
  })
})
</script>

<template>
  <div :aria-busy="loading" class="calendar">
    <div v-for="name in WEEKDAYS" :key="name" class="weekday muted">{{ name }}</div>

    <div v-for="blank in lead" :key="`lead-${blank}`" aria-hidden="true" class="cell blank" />

    <template v-for="slot in days" :key="slot.date">
      <RouterLink
        v-if="slot.entry"
        :class="{ faded: dimmed(slot.date) }"
        :title="slot.entry.title"
        :to="`/${slot.date}`"
        class="cell filled"
      >
        <img
          v-if="slot.entry.media.thumb_url"
          :src="slot.entry.media.thumb_url"
          alt=""
          aria-hidden="true"
          decoding="async"
          loading="lazy"
        />
        <span class="day">{{ slot.day }}</span>
        <span v-if="!isRead(slot.date)" aria-hidden="true" class="unread-dot" />
        <span class="sr-only">
          {{ slot.entry.title }}, {{ isRead(slot.date) ? 'read' : 'unread' }}
        </span>
      </RouterLink>

      <div v-else class="cell empty">
        <Skeleton v-if="loading" height="100%" width="100%" />
        <span v-else class="day muted">{{ slot.day }}</span>
      </div>
    </template>
  </div>
</template>

<style scoped>
.calendar {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  gap: 0.35rem;
}

.weekday {
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  text-align: center;
  padding-bottom: 0.2rem;
}

.cell {
  position: relative;
  aspect-ratio: 1;
  border-radius: 0.5rem;
  overflow: hidden;
  display: block;
}

.blank {
  border: 0;
}

.empty {
  border: 1px dashed var(--border);
  display: grid;
  place-items: center;
}

.filled {
  border: 1px solid var(--border);
  text-decoration: none;
  color: inherit;
  background: color-mix(in srgb, var(--text) 6%, transparent);
  transition:
    transform 0.15s ease,
    border-color 0.15s ease;
}

.filled:hover,
.filled:focus-visible {
  transform: translateY(-2px);
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
}

.filled img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.day {
  position: absolute;
  left: 0.25rem;
  top: 0.15rem;
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
  line-height: 1.4;
}

.filled .day {
  color: #fff;
  padding-inline: 0.2rem;
  border-radius: 0.25rem;
  background: rgb(0 0 0 / 0.45);
}

.unread-dot {
  position: absolute;
  right: 0.28rem;
  bottom: 0.28rem;
  width: 0.42rem;
  height: 0.42rem;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 0 2px rgb(0 0 0 / 0.35);
}

.filled.faded {
  opacity: 0.45;
}

.filled.faded:hover,
.filled.faded:focus-visible {
  opacity: 1;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}

@media (max-width: 30rem) {
  .calendar {
    gap: 0.2rem;
  }

  .cell {
    border-radius: 0.35rem;
  }

  .day {
    font-size: 0.62rem;
    left: 0.1rem;
  }

  .unread-dot {
    width: 0.34rem;
    height: 0.34rem;
    right: 0.18rem;
    bottom: 0.18rem;
  }
}
</style>
