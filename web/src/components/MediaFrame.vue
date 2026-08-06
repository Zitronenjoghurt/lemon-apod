<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { isImage, type Media } from '@/api/types'

const props = defineProps<{
  media: Media
  title: string
  maxHeight?: string
}>()

const SLOW_AFTER_MS = 6000

const playing = ref(false)
const loaded = ref(false)
const failed = ref(false)
const slow = ref(false)
let slowTimer: ReturnType<typeof setTimeout> | undefined

function stopSlowTimer() {
  clearTimeout(slowTimer)
  slowTimer = undefined
}

watch(
  () => props.media.url,
  () => {
    playing.value = false
    loaded.value = false
    failed.value = false
    slow.value = false

    stopSlowTimer()
    if (isImage(props.media.kind) && props.media.url) {
      slowTimer = setTimeout(() => (slow.value = true), SLOW_AFTER_MS)
    }
  },
  { immediate: true },
)

watch([loaded, failed], ([done, broken]) => {
  if (done || broken) stopSlowTimer()
})

onBeforeUnmount(stopSlowTimer)

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

const showsImage = computed(() => isImage(props.media.kind) && !!props.media.url && !failed.value)
</script>

<template>
  <figure class="media" :style="maxHeight ? { '--media-max': maxHeight } : undefined">
    <Image
      v-if="showsImage"
      preview
      class="shot"
      :pt="{ toolbar: { class: 'shot-toolbar' } }"
      :alt="title"
    >
      <template #image>
        <div class="frame">
          <img
            v-if="media.thumb_url && !loaded"
            :src="media.thumb_url"
            alt=""
            aria-hidden="true"
            class="placeholder"
          />
          <img
            :key="media.url ?? ''"
            :src="media.url ?? ''"
            :alt="title"
            fetchpriority="high"
            decoding="async"
            class="full"
            :class="{ ready: loaded }"
            @load="loaded = true"
            @error="failed = true"
          />
          <div v-if="!loaded" class="loading" role="status">
            <span class="badge-loading">
              <span class="spinner" aria-hidden="true" />
              <span class="text">
                Loading full image from NASA
                <span v-if="slow" class="sub">This one is large, still downloading</span>
              </span>
            </span>
          </div>
        </div>
      </template>

      <template #original="{ style, previewCallback }">
        <img
          :src="fullResolution ?? media.url ?? ''"
          :alt="title"
          :style="style"
          class="original"
          @click="previewCallback"
        />
      </template>

      <template #previewicon>
        <i class="pi pi-search-plus" aria-hidden="true" />
      </template>
    </Image>

    <video
      v-else-if="media.kind === 'video_mp4' && media.url"
      class="frame"
      controls
      playsinline
      preload="none"
      :poster="media.thumb_url ?? undefined"
    >
      <source :src="media.url" type="video/mp4" />
    </video>

    <template v-else-if="(media.kind === 'youtube' || media.kind === 'vimeo') && videoId">
      <iframe
        v-if="playing"
        class="frame embed"
        :src="embedUrl"
        :title="title"
        allow="accelerometer; autoplay; encrypted-media; gyroscope; picture-in-picture"
        allowfullscreen
      />
      <button v-else type="button" class="frame facade" @click="playing = true">
        <img v-if="media.thumb_url" :src="media.thumb_url" :alt="title" loading="lazy" />
        <span class="play" aria-hidden="true"><i class="pi pi-play" /></span>
        <span class="sr-only">Play video</span>
      </button>
    </template>

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
  --media-max: min(62vh, 40rem);
}

@media (max-width: 61.99rem) {
  .media {
    --media-max: 46vh;
  }
}

.frame {
  display: block;
  width: 100%;
  max-height: var(--media-max);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  overflow: hidden;
  background: color-mix(in srgb, var(--bg-elevated) 30%, var(--bg));
  position: relative;
}

video.frame,
.embed,
.facade,
.placeholder-card {
  aspect-ratio: 16 / 9;
  height: auto;
}

.frame img,
video.frame,
.frame iframe {
  display: block;
  width: 100%;
  height: 100%;
  max-height: var(--media-max);
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

.loading {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 0.75rem;
  pointer-events: none;
  animation: fade-in 0.2s ease 0.4s both;
}

.badge-loading {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  max-width: min(100%, 20rem);
  padding: 0.6rem 0.9rem;
  border-radius: 0.7rem;
  background: rgb(8 10 20 / 0.68);
  backdrop-filter: blur(6px);
  box-shadow: 0 2px 12px rgb(0 0 0 / 0.3);
  color: #fff;
  text-align: left;
  text-wrap: balance;
}

.text {
  font-size: clamp(0.85rem, 2.8vw, 0.95rem);
  line-height: 1.3;
}

.sub {
  display: block;
  margin-top: 0.15rem;
  font-size: 0.9em;
  color: rgb(255 255 255 / 0.72);
}

.spinner {
  flex: none;
  width: 1.15rem;
  height: 1.15rem;
  border-radius: 50%;
  border: 2px solid rgb(255 255 255 / 0.3);
  border-top-color: #fff;
  animation: spin 0.7s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@keyframes fade-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@media (prefers-reduced-motion: reduce) {
  .spinner {
    animation: none;
  }
}

.original {
  max-width: 95vw;
  max-height: 95vh;
  object-fit: contain;
}

.facade {
  padding: 0;
  cursor: pointer;
}

.facade img {
  object-fit: cover;
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

<style>
.shot {
  display: block;
  position: relative;
  border-radius: var(--radius);
}

.shot .p-image-preview-mask {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: var(--radius);
  background: rgb(8 10 20 / 0.55);
  color: #fff;
  font-size: 1.5rem;
  cursor: zoom-in;
  opacity: 0;
  transition: opacity 0.2s ease;
}

.shot .p-image-preview-mask:hover,
.shot .p-image-preview-mask:focus-visible {
  opacity: 1;
}

.shot-toolbar {
  gap: 0.35rem;
}

.p-image-mask {
  background: rgb(4 5 12 / 0.94);
  backdrop-filter: blur(4px);
}
</style>
