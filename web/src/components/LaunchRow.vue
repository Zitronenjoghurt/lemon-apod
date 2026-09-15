<script lang="ts" setup>
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import type { Launch } from '@/api/types'
import { clockOf, launchMark, statusTone, summaryOf, windowOf } from '@/utils/launches'

const props = defineProps<{ launch: Launch; flown?: boolean }>()

const summary = computed(() => summaryOf(props.launch))
const span = computed(() => windowOf(props.launch))
const tone = computed(() => statusTone(props.launch.status))
const mark = computed(() => launchMark(props.launch, Date.now()))
</script>

<template>
  <RouterLink :class="['launch-row', { flown }]" :to="`/launches/${launch.id}`">
    <img
      v-if="launch.image_url"
      :alt="launch.name"
      :src="launch.image_url"
      class="shot"
      decoding="async"
      height="44"
      loading="lazy"
      width="66"
    />
    <span v-else aria-hidden="true" class="shot fallback">
      <AppIcon :name="mark" />
    </span>

    <span class="when">{{ clockOf(launch) }}</span>

    <span class="what">
      <span class="title">{{ launch.name }}</span>
      <span v-if="summary" class="muted detail">{{ summary }}</span>
    </span>

    <span class="tail">
      <span v-if="launch.webcast_live" class="live">
        <AppIcon name="video" />
        {{ launch.status ?? 'Live' }}
      </span>
      <span v-else-if="flown || (tone && tone !== 'good')" :class="['flag', tone]">
        {{ launch.status }}
      </span>
      <span v-else-if="span" class="muted span">{{ span }}</span>
      <AppIcon name="chevron-right" class="go" />
    </span>
  </RouterLink>
</template>

<style scoped>
.launch-row {
  display: grid;
  grid-template-columns: auto 5rem minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  color: inherit;
  text-decoration: none;
  transition:
    border-color var(--dur-fast) var(--ease-out),
    transform var(--dur-fast) var(--ease-out);
}

.launch-row:hover,
.launch-row:focus-visible {
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  transform: translateY(-1px);
}

.flown {
  opacity: 0.72;
}

.shot {
  width: 4.1rem;
  height: 2.75rem;
  object-fit: cover;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--bg);
}

.fallback {
  display: grid;
  place-items: center;
  color: var(--text-muted);
}

.when {
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.what {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.title {
  font-size: var(--text-sm);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.detail {
  font-size: var(--text-2xs);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.tail {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.flag,
.span {
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 11rem;
}

.span {
  font-size: var(--text-2xs);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.flag {
  flex: none;
  padding: 0 var(--space-2);
  border-radius: var(--radius-pill);
  border: 1px solid currentcolor;
  font-size: var(--text-2xs);
  white-space: nowrap;
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

.flown .flag {
  border-color: var(--border);
  color: var(--text-muted);
}

.flown .flag.bad {
  border-color: var(--bad);
  color: var(--bad);
}

.live {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 0 var(--space-2);
  border-radius: var(--radius-pill);
  border: 1px solid color-mix(in srgb, var(--bad) 45%, var(--border));
  background: color-mix(in srgb, var(--bad) 16%, transparent);
  font-size: var(--text-2xs);
  white-space: nowrap;
}

.go {
  flex: none;
  color: var(--text-muted);
  font-size: var(--text-xs);
}

@media (max-width: 40rem) {
  .launch-row {
    grid-template-columns: auto auto minmax(0, 1fr) auto;
    column-gap: var(--space-2);
    row-gap: 0;
  }

  .shot {
    grid-area: 1 / 1 / 3 / 2;
  }

  .what {
    display: contents;
  }

  .title {
    grid-area: 1 / 2 / 2 / 4;
    min-width: 0;
  }

  .when {
    grid-area: 2 / 2 / 3 / 3;
    font-size: var(--text-2xs);
  }

  .detail {
    grid-area: 2 / 3 / 3 / 4;
    min-width: 0;
  }

  .tail {
    grid-area: 1 / 4 / 3 / 5;
  }

  .flag:not(.bad):not(.warn) {
    display: none;
  }

  .flag {
    max-width: 8rem;
  }

  .span {
    display: none;
  }
}
</style>
