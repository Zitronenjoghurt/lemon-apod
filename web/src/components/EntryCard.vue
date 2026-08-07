<script lang="ts" setup>
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { type ApodSummary, isVideo } from '@/api/types'
import { useRead } from '@/composables/useRead'
import { formatDate } from '@/utils/date'

const props = defineProps<{
  entry: ApodSummary
  snippet?: string
  query?: string
}>()

const { isRead, dimmed } = useRead()

const unread = computed(() => !isRead(props.entry.date))
const faded = computed(() => dimmed(props.entry.date))

const target = computed(() =>
  props.query?.trim()
    ? { path: `/${props.entry.date}`, query: { q: props.query } }
    : `/${props.entry.date}`,
)
</script>

<template>
  <RouterLink :class="{ faded }" :to="target" class="card entry-card">
    <div class="thumb">
      <img
        v-if="entry.media.thumb_url"
        :alt="entry.title"
        :src="entry.media.thumb_url"
        decoding="async"
        height="300"
        loading="lazy"
        width="480"
      />
      <div v-else class="fallback">
        <i aria-hidden="true" class="pi pi-image" />
      </div>
      <span v-if="isVideo(entry.media.kind)" aria-label="Video" class="badge">
        <i aria-hidden="true" class="pi pi-play" />
      </span>
    </div>

    <div class="body">
      <p class="muted date">
        <span v-if="unread" aria-hidden="true" class="unread-dot" />
        <time :datetime="entry.date">{{ formatDate(entry.date) }}</time>
        <span class="sr-only">{{ unread ? 'Unread' : 'Read' }}</span>
      </p>
      <h3 class="title">{{ entry.title }}</h3>
      <p v-if="snippet" class="snippet muted" v-html="snippet" />
    </div>
  </RouterLink>
</template>

<style scoped>
.entry-card {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  text-decoration: none;
  color: inherit;
  transition:
    transform 0.18s ease,
    box-shadow 0.18s ease,
    border-color 0.18s ease;
}

.entry-card:hover {
  transform: translateY(-2px);
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
  box-shadow: 0 8px 28px rgb(0 0 0 / 0.18);
}

.thumb {
  position: relative;
  aspect-ratio: 16 / 10;
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.fallback {
  width: 100%;
  height: 100%;
  display: grid;
  place-items: center;
  color: var(--text-muted);
  font-size: 1.6rem;
}

.badge {
  position: absolute;
  right: 0.6rem;
  bottom: 0.6rem;
  width: 2rem;
  height: 2rem;
  border-radius: 50%;
  background: rgb(0 0 0 / 0.6);
  color: #fff;
  display: grid;
  place-items: center;
  font-size: 0.75rem;
  padding-left: 0.15rem;
}

.unread-dot {
  display: inline-block;
  width: 0.4rem;
  height: 0.4rem;
  border-radius: 50%;
  background: var(--accent);
  margin-right: 0.4rem;
  vertical-align: 0.08em;
}

.entry-card.faded {
  opacity: 0.55;
}

.entry-card.faded:hover,
.entry-card.faded:focus-within {
  opacity: 1;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}

.body {
  padding: 0.85rem 1rem 1.1rem;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.date {
  font-size: 0.8rem;
  letter-spacing: 0.02em;
  margin: 0;
}

.title {
  font-size: 1.02rem;
  font-weight: 600;
}

.snippet {
  font-size: 0.88rem;
  margin: 0.15rem 0 0;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
