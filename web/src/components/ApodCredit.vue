<script lang="ts" setup>
import { computed } from 'vue'
import { APOD_URL } from '@/utils/links'

const props = withDefaults(
  defineProps<{
    variant?: 'caption' | 'banner' | 'overlay'
    source?: string
    lead?: string
    title?: string
  }>(),
  {
    variant: 'caption',
    source: undefined,
    lead: "Every picture here is from NASA's",
    title: undefined,
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

  <p v-else-if="variant === 'overlay'" class="apod-credit overlay">
    <span class="kicker">NASA's</span>
    <span class="name">Astronomy Picture of the Day</span>
    <span v-if="title" class="work">{{ title }}</span>
  </p>

  <div v-else class="apod-credit caption">
    <span class="head">
      <i aria-hidden="true" class="pi pi-image mark" />
      <span class="stack-text">
        <span class="kicker">From NASA's</span>
        <a :href="href" class="name" rel="noopener" target="_blank">
          Astronomy Picture of the Day
          <i aria-hidden="true" class="pi pi-external-link away" />
        </a>
      </span>
    </span>

    <div v-if="$slots.default" class="detail">
      <slot />
    </div>
  </div>
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

a.name:hover,
a.name:focus-visible {
  color: var(--accent);
  text-decoration: underline;
}

.away {
  font-size: 0.7em;
  vertical-align: 0.08em;
  margin-left: 0.15rem;
  color: var(--text-muted);
}

.kicker {
  font-size: 0.72rem;
  letter-spacing: 0.09em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.mark {
  flex: none;
  font-size: 0.95rem;
  color: var(--accent);
}

.caption {
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
  padding: 0.6rem 0.85rem 0.65rem;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--accent) 6%, transparent);
}

.caption .head {
  display: flex;
  align-items: center;
  gap: 0.7rem;
}

.caption .stack-text {
  display: flex;
  flex-direction: column;
  gap: 0.05rem;
  min-width: 0;
}

.caption .name {
  font-size: 1.02rem;
  line-height: 1.25;
}

.detail {
  padding-top: 0.55rem;
  border-top: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
}

.banner {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
  font-size: 0.95rem;
  color: var(--text-muted);
  text-wrap: pretty;
}

.banner .mark {
  align-self: center;
}

.overlay {
  display: flex;
  flex-direction: column;
  gap: 0.05rem;
  padding: 0.5rem 0.9rem;
  border-radius: 0.7rem;
  background: rgb(8 10 20 / 0.72);
  backdrop-filter: blur(6px);
  text-align: left;
}

.overlay .kicker {
  color: rgb(255 255 255 / 0.7);
}

.overlay .name {
  font-size: 0.98rem;
  color: #fff;
}

.overlay .work {
  font-size: 0.88rem;
  color: rgb(255 255 255 / 0.78);
  text-wrap: balance;
}
</style>
