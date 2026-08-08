<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { api } from '@/api/client'
import type { GamePicture } from '@/api/types'

const props = withDefaults(
  defineProps<{
    picture: GamePicture
    blur?: number
    alt?: string
    state?: 'plain' | 'picked' | 'right' | 'wrong'
    frame?: number
  }>(),
  { blur: 0, alt: 'A picture from the archive', state: 'plain', frame: 0 },
)

const loaded = ref(false)
const failed = ref(false)

const source = computed(() => api.games.picture(props.picture.picture))
const ratio = computed(() => {
  const { width, height } = props.picture
  return width && height ? width / height : 1
})

const scale = computed(() => 1 + props.blur / 22)

watch(source, () => {
  loaded.value = false
  failed.value = false
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
  </div>
</template>

<style scoped>
.game-picture {
  position: relative;
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
</style>
