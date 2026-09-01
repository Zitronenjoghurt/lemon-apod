<script lang="ts" setup>
import { computed } from 'vue'
import { APOD_URL } from '@/utils/links'

const props = withDefaults(
  defineProps<{
    variant?: 'caption' | 'banner'
    source?: string
    lead?: string
  }>(),
  {
    variant: 'caption',
    source: undefined,
    lead: "Every picture here is from NASA's",
  },
)

const href = computed(() => props.source || APOD_URL)
</script>

<template>
  <p v-if="variant === 'banner'" class="apod-credit banner">
    <i aria-hidden="true" class="pi pi-image mark" />
    <span class="line">
      {{ lead }}
      <a :href="href" class="name" rel="noopener" target="_blank">
        Astronomy Picture of the Day
        <i aria-hidden="true" class="pi pi-external-link away" />
      </a>
    </span>
  </p>

  <p v-else class="apod-credit caption">
    <i aria-hidden="true" class="pi pi-image mark" />
    <span class="stack-text">
      <span class="kicker">From NASA's</span>
      <a :href="href" class="name" rel="noopener" target="_blank">
        Astronomy Picture of the Day
        <i aria-hidden="true" class="pi pi-external-link away" />
      </a>
    </span>
  </p>
</template>

<style scoped>
.apod-credit {
  margin: 0;
}

.name {
  font-weight: 600;
  color: var(--text);
  text-decoration: none;
  text-wrap: balance;
}

a.name {
  transition: color var(--dur-fast) var(--ease-out);
}

a.name:hover,
a.name:focus-visible {
  color: var(--accent);
  text-decoration: underline;
}

.away {
  font-size: 0.7em;
  vertical-align: 0.08em;
  margin-left: var(--space-0);
  color: var(--text-muted);
}

.kicker {
  font-size: var(--text-xs);
  letter-spacing: 0.09em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.mark {
  flex: none;
  font-size: var(--text-md);
  color: var(--accent);
}

.caption {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-width: 0;
}

.caption .stack-text {
  display: flex;
  flex-direction: column;
  gap: var(--space-0);
  min-width: 0;
}

.caption .name {
  font-size: var(--text-md);
  line-height: 1.25;
}

.banner {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  font-size: var(--text-md);
  color: var(--text-muted);
  text-wrap: pretty;
}

.banner .mark {
  align-self: center;
}
</style>
