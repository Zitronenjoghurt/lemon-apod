<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { isVideo, type ApodSummary } from '@/api/types'
import { formatDate } from '@/utils/date'

defineProps<{ entry: ApodSummary; snippet?: string }>()
</script>

<template>
  <RouterLink :to="`/${entry.date}`" class="card entry-card">
    <div class="thumb">
      <img
        v-if="entry.media.thumb_url"
        :src="entry.media.thumb_url"
        :alt="entry.title"
        loading="lazy"
        decoding="async"
        width="480"
        height="300"
      />
      <div v-else class="fallback">
        <i class="pi pi-image" aria-hidden="true" />
      </div>
      <span v-if="isVideo(entry.media.kind)" class="badge" aria-label="Video">
        <i class="pi pi-play" aria-hidden="true" />
      </span>
    </div>

    <div class="body">
      <time :datetime="entry.date" class="muted date">{{ formatDate(entry.date) }}</time>
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

.body {
  padding: 0.85rem 1rem 1.1rem;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.date {
  font-size: 0.8rem;
  letter-spacing: 0.02em;
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
