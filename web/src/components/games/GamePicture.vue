<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
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
const fullLoaded = ref(false)

const source = computed(() => api.games.picture(props.picture.picture))
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

watch(zoomed, (open) => {
  if (!open) fullLoaded.value = false
})
</script>

<template>
  <div :class="['game-picture', state, { framed: frame }]" :style="{ '--ratio': frame || ratio }">
    <Skeleton v-if="!loaded && !failed" class="fill" height="100%" width="100%" />
    <p v-if="failed" class="muted gone">
      <i aria-hidden="true" class="pi pi-image" />
      This picture could not be loaded.
    </p>
    <img
      v-show="loaded"
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

  <Dialog
    v-if="zoomable"
    v-model:visible="zoomed"
    :header="alt"
    :style="{ width: 'min(96rem, 96vw)' }"
    dismissable-mask
    modal
  >
    <div class="full-wrap">
      <Skeleton v-if="!fullLoaded" height="60vh" width="100%" />
      <img
        v-show="fullLoaded"
        :alt="alt"
        :src="full ?? undefined"
        class="full"
        decoding="async"
        @load="fullLoaded = true"
      />
      <RouterLink v-if="date" :to="`/${date}`" class="muted open-entry">
        Open this entry
        <i aria-hidden="true" class="pi pi-angle-right" />
      </RouterLink>
    </div>
  </Dialog>
</template>

<style scoped>
.game-picture {
  position: relative;
  container-type: inline-size;
  overflow: hidden;
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background: var(--bg-elevated);
  font-size: min(4vw, 1.4rem);
  margin-inline: auto;
  width: 100%;
  aspect-ratio: var(--ratio);
  max-width: calc(var(--cap, 200vh) * var(--ratio));
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
  gap: 0.4rem;
  justify-items: center;
  font-size: 0.85rem;
  margin: 0;
  padding: 1rem;
  text-align: center;
}

.picked {
  outline: 3px solid var(--accent);
  outline-offset: 2px;
}

.right {
  outline: 3px solid #16a34a;
  outline-offset: 2px;
}

.wrong {
  outline: 3px solid #dc2626;
  outline-offset: 2px;
  opacity: 0.75;
}

.zoom {
  position: absolute;
  right: 0.45rem;
  bottom: 0.45rem;
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.2rem 0.5rem;
  border: 0;
  border-radius: 999px;
  background: rgb(0 0 0 / 0.6);
  color: #fff;
  font: inherit;
  font-size: 0.7rem;
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

.full-wrap {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  align-items: flex-start;
}

.full {
  display: block;
  width: 100%;
  height: auto;
  max-height: 78vh;
  object-fit: contain;
  border-radius: calc(var(--radius) / 2);
}

.open-entry {
  display: inline-flex;
  align-items: center;
  gap: 0.15rem;
  font-size: 0.85rem;
  text-decoration: none;
}

.open-entry:hover {
  text-decoration: underline;
}
</style>
