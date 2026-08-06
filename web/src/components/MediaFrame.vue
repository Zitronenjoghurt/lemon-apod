<script setup lang="ts">
import { computed, ref } from 'vue'
import { isImage, type Media } from '@/api/types'

const props = withDefaults(
  defineProps<{
    media: Media
    title: string
    linkFullResolution?: boolean
  }>(),
  { linkFullResolution: true },
)

const playing = ref(false)
const loaded = ref(false)
const failed = ref(false)

const videoId = computed(() => {
  const path = (props.media.url ?? '').split(/[?#]/)[0] ?? ''

  if (props.media.kind === 'youtube') {
    return path.split('/embed/').pop() ?? ''
  }
  if (props.media.kind === 'vimeo') {
    return path.split('/video/').pop() ?? ''
  }
  return ''
})

const embedUrl = computed(() => {
  if (props.media.kind === 'youtube') {
    return `https://www.youtube-nocookie.com/embed/${videoId.value}?autoplay=1&rel=0`
  }
  if (props.media.kind === 'vimeo') {
    return `https://player.vimeo.com/video/${videoId.value}?autoplay=1&dnt=1`
  }
  return ''
})

const fullResolution = computed(() => {
  const { hd_url: hd, url } = props.media
  return hd && hd !== url ? hd : null
})
</script>

<template>
  <figure class="media">
    <template v-if="isImage(media.kind) && media.url && !failed">
      <component
        :is="linkFullResolution && fullResolution ? 'a' : 'div'"
        :href="fullResolution ?? undefined"
        target="_blank"
        rel="noopener"
        class="frame"
        :class="{ zoomable: linkFullResolution && fullResolution }"
      >
        <img
          v-if="media.thumb_url && !loaded"
          :src="media.thumb_url"
          :alt="''"
          aria-hidden="true"
          class="placeholder"
        />
        <img
          :src="media.url"
          :alt="title"
          loading="lazy"
          decoding="async"
          class="full"
          :class="{ ready: loaded }"
          @load="loaded = true"
          @error="failed = true"
        />
      </component>
      <figcaption v-if="fullResolution && linkFullResolution" class="hint muted">
        <i class="pi pi-search-plus" aria-hidden="true" /> Open full resolution
      </figcaption>
    </template>

    <video
      v-else-if="media.kind === 'video_mp4' && media.url"
      class="frame"
      controls
      preload="none"
      :poster="media.thumb_url ?? undefined"
    >
      <source :src="media.url" type="video/mp4" />
    </video>

    <template v-else-if="(media.kind === 'youtube' || media.kind === 'vimeo') && videoId">
      <iframe
        v-if="playing"
        class="frame"
        :src="embedUrl"
        :title="title"
        allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
        allowfullscreen
      />
      <button v-else type="button" class="frame facade" @click="playing = true">
        <img v-if="media.thumb_url" :src="media.thumb_url" :alt="title" loading="lazy" />
        <span class="play" aria-hidden="true"><i class="pi pi-play" /></span>
        <span class="sr-only">Play video</span>
      </button>
    </template>

    <!-- Interactive embeds, applets and the handful of pages with no media at all -->
    <a
      v-else
      class="frame placeholder-card"
      :href="media.url ?? '#'"
      target="_blank"
      rel="noopener"
    >
      <i class="pi pi-external-link" aria-hidden="true" />
      <span>{{ media.kind === 'none' ? 'No media on this entry' : 'View on apod.nasa.gov' }}</span>
    </a>
  </figure>
</template>

<style scoped>
.media {
  margin: 0;
}

.frame {
  display: block;
  width: 100%;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  overflow: hidden;
  background: var(--bg-elevated);
  position: relative;
  aspect-ratio: 16 / 10;
}

.frame img,
.frame video,
.frame iframe {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
  border: 0;
}

.placeholder {
  position: absolute;
  inset: 0;
  filter: blur(14px);
  transform: scale(1.06);
}

.full {
  position: relative;
  opacity: 0;
  transition: opacity 0.35s ease;
}

.full.ready {
  opacity: 1;
}

.zoomable {
  cursor: zoom-in;
}

.hint {
  font-size: 0.85rem;
  margin-top: 0.5rem;
  display: flex;
  align-items: center;
  gap: 0.35rem;
}

.facade {
  padding: 0;
  cursor: pointer;
  border: 1px solid var(--border);
}

.facade .play {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
}

.facade .play i {
  font-size: 1.5rem;
  color: #fff;
  background: rgb(0 0 0 / 0.55);
  backdrop-filter: blur(4px);
  border-radius: 50%;
  width: 4rem;
  height: 4rem;
  display: grid;
  place-items: center;
  padding-left: 0.25rem;
  transition:
    transform 0.2s ease,
    background 0.2s ease;
}

.facade:hover .play i {
  transform: scale(1.08);
  background: rgb(0 0 0 / 0.75);
}

.placeholder-card {
  display: grid;
  place-content: center;
  justify-items: center;
  gap: 0.6rem;
  color: var(--text-muted);
  text-decoration: none;
  aspect-ratio: 16 / 7;
}

.placeholder-card i {
  font-size: 1.5rem;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}
</style>
