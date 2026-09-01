<script lang="ts" setup>
import { computed, onBeforeUnmount, ref, useTemplateRef, watch } from 'vue'
import ApodCredit from './ApodCredit.vue'
import MediaLightbox from './MediaLightbox.vue'
import type { Slide } from './MediaLightbox.vue'
import { aspectRatio, isImage, isLost, isUndisplayableImage, type Media } from '@/api/types'

const props = defineProps<{
  media: Media
  title: string
  maxHeight?: string
  source?: string
  entry?: string
}>()

const SLOW_AFTER_MS = 6000

const playing = ref(false)
const zoomed = ref(false)
const inline = useTemplateRef<HTMLImageElement>('inline')

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

const placeholderLabel = computed(() => {
  if (props.media.kind === 'none') return 'No media on this entry'
  if (props.media.kind === 'embed') return 'Open the interactive embed'
  return 'Open the original file'
})

const lost = computed(() => isLost(props.media))

const undisplayable = computed(
  () => isUndisplayableImage(props.media.kind) && !!props.media.thumb_url,
)

function slide(): Slide | null {
  const element = inline.value
  if (!element?.naturalWidth || !props.media.url) return null

  return {
    src: props.media.url,
    width: element.naturalWidth,
    height: element.naturalHeight,
    alt: props.title,
    hd: fullResolution.value ?? undefined,
    thumb: props.media.thumb_url ?? undefined,
    entry: props.entry,
    source: props.source,
    from: () => inline.value,
  }
}

const alone = computed(() => {
  if (!loaded.value) return []

  const one = slide()
  return one ? [one] : []
})

const showsImage = computed(() => isImage(props.media.kind) && !!props.media.url && !failed.value)
const ratio = computed(() => aspectRatio(props.media))
const frameStyle = computed(() => ({
  ...(props.maxHeight ? { '--media-max': props.maxHeight } : {}),
  ...(ratio.value ? { '--media-ratio': String(ratio.value) } : {}),
}))
</script>

<template>
  <figure :style="frameStyle" class="media">
    <figcaption class="source">
      <ApodCredit :source="source" variant="caption" />
    </figcaption>

    <button
      v-if="showsImage"
      :aria-label="`View ${title} at full size`"
      class="shot"
      type="button"
      @click="zoomed = true"
    >
      <div :class="{ guessed: !ratio }" class="frame reserved">
        <img
          v-if="media.thumb_url && !loaded"
          :src="media.thumb_url"
          alt=""
          aria-hidden="true"
          class="placeholder"
        />
        <img
          :key="media.url ?? ''"
          ref="inline"
          :alt="title"
          :class="{ ready: loaded }"
          :src="media.url ?? ''"
          class="full"
          decoding="async"
          fetchpriority="high"
          @error="failed = true"
          @load="loaded = true"
        />
        <div v-if="!loaded" class="loading" role="status">
          <span class="badge-loading">
            <span aria-hidden="true" class="spinner" />
            <span class="text">
              Loading full image from NASA
              <span v-if="slow" class="sub">This one is large, still downloading</span>
            </span>
          </span>
        </div>
      </div>
      <span aria-hidden="true" class="magnify"><i class="pi pi-search-plus" /></span>
    </button>

    <template v-else-if="undisplayable">
      <div class="frame reserved">
        <img :alt="title" :src="media.thumb_url ?? ''" class="full ready" decoding="async" />
      </div>
      <p class="tiff-note">
        NASA's copy of this picture is a TIFF, which browsers do not display. The image above is the
        thumbnail generated from it.
        <a :href="media.url ?? '#'" rel="noopener" target="_blank">Open the original file</a>.
      </p>
    </template>

    <video
      v-else-if="media.kind === 'video_mp4' && media.url"
      :poster="media.thumb_url ?? undefined"
      class="frame"
      controls
      playsinline
      preload="none"
    >
      <source :src="media.url" type="video/mp4" />
    </video>

    <template v-else-if="(media.kind === 'youtube' || media.kind === 'vimeo') && videoId">
      <iframe
        v-if="playing"
        :src="embedUrl"
        :title="title"
        allow="accelerometer; autoplay; encrypted-media; gyroscope; picture-in-picture"
        allowfullscreen
        class="frame embed"
      />
      <button v-else class="frame facade" type="button" @click="playing = true">
        <img v-if="media.thumb_url" :alt="title" :src="media.thumb_url" loading="lazy" />
        <span aria-hidden="true" class="play"><i class="pi pi-play" /></span>
        <span class="sr-only">Play video</span>
      </button>
    </template>

    <div v-else-if="lost" class="frame placeholder-card gone">
      <i aria-hidden="true" class="pi pi-ban" />
      <span>Media lost</span>
      <small>
        The media this entry referenced became unreachable before this archive was able to preserve
        it.
      </small>
    </div>

    <a
      v-else
      :href="media.url ?? '#'"
      class="frame placeholder-card"
      rel="noopener"
      target="_blank"
    >
      <i aria-hidden="true" class="pi pi-external-link" />
      <span>{{ placeholderLabel }}</span>
    </a>

    <MediaLightbox :at="zoomed ? 0 : null" :slides="alone" @close="zoomed = false" />

    <div v-if="$slots.credit" class="credit">
      <slot name="credit" />
    </div>

    <div v-if="$slots.actions" class="doings">
      <slot name="actions" />
    </div>
  </figure>
</template>

<style scoped>
.media {
  margin: 0;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  overflow: hidden;
  background: var(--bg-elevated);
  --media-max: min(62vh, 40rem);
}

.source,
.credit {
  padding: var(--space-2) var(--space-3);
  min-width: 0;
}

.doings {
  padding: var(--space-1) 0;
  border-top: 1px solid var(--border);
  min-width: 0;
}

.source {
  border-bottom: 1px solid var(--border);
}

.credit {
  border-top: 1px solid var(--border);
}

@media (max-width: 61.99rem) {
  .media {
    --media-max: 46vh;
  }
}

.shot {
  display: block;
  position: relative;
  width: 100%;
  padding: 0;
  border: 0;
  background: none;
  cursor: zoom-in;
  line-height: 0;
}

.magnify {
  position: absolute;
  right: var(--space-3);
  bottom: var(--space-3);
  display: grid;
  place-items: center;
  width: 2.25rem;
  height: 2.25rem;
  border-radius: 50%;
  background: rgb(8 10 20 / 0.6);
  backdrop-filter: blur(6px);
  color: #fff;
  font-size: var(--text-sm);
  line-height: 1;
  opacity: 0;
  transform: scale(0.9);
  transition:
    opacity var(--dur-base) var(--ease-out),
    transform var(--dur-base) var(--ease-out);
}

.shot:hover .magnify,
.shot:focus-visible .magnify {
  opacity: 1;
  transform: none;
}

.frame {
  display: block;
  width: 100%;
  max-height: var(--media-max);
  overflow: hidden;
  background: color-mix(in srgb, var(--text) 4%, transparent);
  position: relative;
}

.frame.reserved {
  aspect-ratio: var(--media-ratio, 1);
  height: auto;
  background: linear-gradient(
      100deg,
      transparent 20%,
      color-mix(in srgb, var(--text) 7%, transparent) 40%,
      transparent 60%
    )
    color-mix(in srgb, var(--text) 4%, transparent);
  background-size: 250% 100%;
  animation: shimmer 1.6s linear infinite;
}

.frame.reserved:has(.full.ready) {
  animation: none;
  background: color-mix(in srgb, var(--text) 4%, transparent);
}

.frame.guessed:has(.full.ready) {
  aspect-ratio: auto;
}

@keyframes shimmer {
  from {
    background-position: 150% 0;
  }
  to {
    background-position: -50% 0;
  }
}

@media (prefers-reduced-motion: reduce) {
  .frame.reserved {
    animation: none;
  }
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
  padding: var(--space-3);
  pointer-events: none;
  animation: fade-in 0.2s ease 0.4s both;
}

.badge-loading {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  max-width: min(100%, 20rem);
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-md);
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
  margin-top: var(--space-0);
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
  font-size: var(--text-title);
  color: #fff;
  background: rgb(0 0 0 / 0.55);
  backdrop-filter: blur(4px);
  border-radius: 50%;
  width: 4rem;
  height: 4rem;
  display: grid;
  place-items: center;
  padding-left: var(--space-1);
  transition:
    transform 0.2s ease,
    background 0.2s ease;
}

.facade:hover .play i {
  transform: scale(1.08);
  background: rgb(0 0 0 / 0.75);
}

.tiff-note {
  margin: 0;
  padding: var(--space-2) var(--space-3);
  border-top: 1px solid var(--border);
  font-size: var(--text-sm);
  color: var(--text-muted);
  text-wrap: pretty;
}

.placeholder-card {
  display: grid;
  place-content: center;
  justify-items: center;
  gap: var(--space-2);
  padding-inline: var(--space-5);
  color: var(--text-muted);
  text-decoration: none;
  text-align: center;
}

/* Hatched, so a picture the archive lost cannot be read as one that has not loaded yet. */
.placeholder-card.gone {
  background: repeating-linear-gradient(
    -45deg,
    transparent 0 8px,
    color-mix(in srgb, var(--text) 5%, transparent) 8px 16px
  );
}

.placeholder-card.gone i {
  color: hsl(var(--tone-warn));
}

.placeholder-card small {
  max-width: 44ch;
  font-size: var(--text-xs);
  line-height: 1.5;
  text-wrap: pretty;
}

.placeholder-card i {
  font-size: var(--text-title);
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
}
</style>
