<script lang="ts" setup>
import { computed, ref, useTemplateRef, watch } from 'vue'
import type { Slide } from '@/components/MediaLightbox.vue'
import MediaLightbox from '@/components/MediaLightbox.vue'
import { api } from '@/api/client'
import type { GamePicture } from '@/api/types'

const props = withDefaults(
  defineProps<{
    picture: GamePicture
    blur?: number
    alt?: string
    state?: 'plain' | 'picked' | 'right' | 'wrong'
    frame?: number
    /** The display image, once the round is over. Only ever passed after a reveal: during play
     *  the token is all the client gets, because the source URL carries the date in it. */
    full?: string | null
    /** Where the full view links back to, as YYYY-MM-DD. */
    date?: string
  }>(),
  { blur: 0, alt: 'A picture from the archive', state: 'plain', frame: 0, full: null, date: '' },
)

const loaded = ref(false)
const failed = ref(false)
const zoomed = ref(false)
const shown = useTemplateRef<HTMLImageElement>('shown')
const revealed = ref<{ width: number; height: number } | null>(null)

watch(
  () => props.full,
  (file) => {
    revealed.value = null
    if (!file) return

    const probe = new Image()
    probe.onload = () =>
      (revealed.value = { width: probe.naturalWidth, height: probe.naturalHeight })
    probe.src = file
  },
  { immediate: true },
)

const alone = computed<Slide[]>(() => {
  const size = revealed.value
  if (!props.full || !size) return []

  return [
    {
      src: props.full,
      width: size.width,
      height: size.height,
      alt: props.alt,
      entry: props.date ? `/${props.date}` : undefined,
      from: () => shown.value,
    },
  ]
})

const source = computed(() => api.games.picture(props.picture.picture))
const credit = computed(() => props.picture.credit ?? [])
const ratio = computed(() => {
  const { width, height } = props.picture
  return width && height ? width / height : 1
})

const scale = computed(() => 1 + props.blur / 22)
const zoomable = computed(() => Boolean(props.full) && !props.blur)

watch(source, () => {
  loaded.value = false
  failed.value = false
})
</script>

<template>
  <div :style="{ '--ratio': frame || ratio }" class="shot">
    <div :class="['game-picture', state, { framed: frame }]">
      <Skeleton v-if="!loaded && !failed" class="fill" height="100%" width="100%" />
      <p v-if="failed" class="muted gone">
        <i aria-hidden="true" class="pi pi-image" />
        This picture could not be loaded.
      </p>
      <img
        v-show="loaded"
        ref="shown"
        :alt="alt"
        :src="source"
        :style="{
          filter: blur ? `blur(${blur / 100}em)` : 'none',
          transform: `scale(${scale})`,
        }"
        decoding="async"
        draggable="false"
        @error="failed = true"
        @load="loaded = true"
      />

      <button v-if="zoomable && loaded" class="zoom" type="button" @click="zoomed = true">
        <i aria-hidden="true" class="pi pi-search-plus" />
        <span class="zoom-label">Full size</span>
      </button>
    </div>

    <p v-if="credit.length" class="muted shot-credit">
      <span v-for="line in credit" :key="line">{{ line }}</span>
    </p>
  </div>

  <MediaLightbox v-if="zoomable" :at="zoomed ? 0 : null" :slides="alone" @close="zoomed = false" />
</template>

<style scoped>
.shot {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: 100%;
  margin-inline: auto;
  max-width: calc(var(--cap, 200vh) * var(--ratio));
}

.shot-credit {
  display: flex;
  flex-direction: column;
  gap: var(--space-0);
  margin: 0;
  font-size: var(--text-xs);
  line-height: 1.35;
  text-wrap: pretty;
}

.game-picture {
  position: relative;
  container-type: inline-size;
  overflow: hidden;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--bg-elevated);
  font-size: min(4vw, 1.4rem);
  width: 100%;
  aspect-ratio: var(--ratio);
}

.game-picture img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition:
    filter 0.7s ease,
    transform 0.7s ease;
}

.framed {
  background: color-mix(in srgb, var(--text) 6%, var(--bg-elevated));
}

.framed img {
  object-fit: contain;
}

.fill {
  position: absolute;
  inset: 0;
}

.gone {
  position: absolute;
  inset: 0;
  display: grid;
  place-content: center;
  gap: var(--space-2);
  justify-items: center;
  font-size: var(--text-sm);
  margin: 0;
  padding: var(--space-4);
  text-align: center;
}

.picked {
  outline: 3px solid var(--accent);
  outline-offset: 2px;
}

.right {
  outline: 3px solid var(--good);
  outline-offset: 2px;
}

.wrong {
  outline: 3px solid var(--bad);
  outline-offset: 2px;
  opacity: 0.75;
}

.zoom {
  position: absolute;
  right: 0.45rem;
  bottom: 0.45rem;
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-2);
  border: 0;
  border-radius: var(--radius-pill);
  background: rgb(0 0 0 / 0.6);
  color: #fff;
  font: inherit;
  font-size: var(--text-xs);
  cursor: pointer;
  opacity: 0.75;
  transition: opacity 0.15s ease;
}

.zoom:hover,
.zoom:focus-visible {
  opacity: 1;
}

/* On a small card the label would crowd the picture out; the icon still says what it does. */
@container (max-width: 14rem) {
  .zoom-label {
    display: none;
  }
}
</style>
