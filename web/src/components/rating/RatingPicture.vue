<script lang="ts" setup>
import { computed, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import ApodCredit from '@/components/ApodCredit.vue'
import type { BallotSide } from '@/api/types'
import { formatDate } from '@/utils/date'

const props = withDefaults(
  defineProps<{
    side: BallotSide
    hint?: string
    state?: 'plain' | 'picked' | 'passed'
    disabled?: boolean
  }>(),
  { hint: '', state: 'plain', disabled: false },
)

const emit = defineEmits<{ pick: [] }>()

const loaded = ref(false)
const failed = ref(false)
const zoomed = ref(false)
const fullLoaded = ref(false)

const thumb = computed(() => props.side.media.thumb_url ?? undefined)
const full = computed(() => props.side.media.hd_url ?? props.side.media.url ?? null)
const ratio = computed(() => {
  const { thumb_width: width, thumb_height: height } = props.side.media
  return width && height ? width / height : 4 / 3
})

const reruns = computed(() => props.side.dates.length)

watch(thumb, () => {
  loaded.value = false
  failed.value = false
})

watch(zoomed, (open) => {
  if (!open) fullLoaded.value = false
})
</script>

<template>
  <div :class="['rating-picture', state]" :style="{ '--ratio': String(ratio) }">
    <button
      :aria-label="`Choose ${side.title}`"
      :disabled="disabled"
      class="shot"
      type="button"
      @click="emit('pick')"
    >
      <Skeleton v-if="!loaded && !failed" class="fill" height="100%" width="100%" />
      <span v-if="failed" class="muted gone">
        <i aria-hidden="true" class="pi pi-image" />
        This picture could not be loaded.
      </span>
      <img
        v-show="loaded"
        :alt="side.title"
        :src="thumb"
        decoding="async"
        draggable="false"
        @error="failed = true"
        @load="loaded = true"
      />

      <span v-if="hint" class="key">{{ hint }}</span>
    </button>

    <div class="under">
      <p class="muted line">
        <time :datetime="side.date">{{ formatDate(side.date) }}</time>
        <span v-if="reruns > 1" class="tag">
          <i aria-hidden="true" class="pi pi-replay" />
          {{ reruns }}&times;
        </span>

        <span class="peek">
          <button
            v-if="full"
            v-tooltip.bottom="'See it full size'"
            class="icon"
            type="button"
            @click="zoomed = true"
          >
            <i aria-hidden="true" class="pi pi-search-plus" />
            <span class="sr-only">See {{ side.title }} full size</span>
          </button>
          <RouterLink v-tooltip.bottom="'Read this entry'" :to="`/${side.date}`" class="icon">
            <i aria-hidden="true" class="pi pi-book" />
            <span class="sr-only">Read the entry for {{ side.title }}</span>
          </RouterLink>
        </span>
      </p>

      <p v-if="side.credit?.length" class="muted credit">
        <span v-for="line in side.credit" :key="line">{{ line }}</span>
      </p>
    </div>
  </div>

  <Dialog
    v-if="full"
    v-model:visible="zoomed"
    :header="side.title"
    :style="{ width: 'min(96rem, 96vw)' }"
    dismissable-mask
    modal
  >
    <div class="full-wrap">
      <ApodCredit :source="side.source_url" lead="This picture is from NASA's" variant="banner" />

      <Skeleton v-if="!fullLoaded" height="60vh" width="100%" />
      <img
        v-show="fullLoaded"
        :alt="side.title"
        :src="full"
        class="full"
        decoding="async"
        @load="fullLoaded = true"
      />

      <p v-if="side.credit?.length" class="muted credit">
        <span v-for="line in side.credit" :key="line">{{ line }}</span>
      </p>

      <RouterLink :to="`/${side.date}`" class="muted open-entry">
        Read the whole entry
        <i aria-hidden="true" class="pi pi-angle-right" />
      </RouterLink>
    </div>
  </Dialog>
</template>

<style scoped>
.rating-picture {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  min-width: 0;
  max-width: calc(var(--cap, 200vh) * var(--ratio));
  margin-inline: auto;
  width: 100%;
}

.shot {
  position: relative;
  display: block;
  width: 100%;
  aspect-ratio: var(--ratio);
  padding: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-elevated);
  overflow: hidden;
  cursor: pointer;
  transition:
    transform 0.12s ease,
    border-color 0.12s ease,
    box-shadow 0.12s ease;
}

.shot:hover:not(:disabled),
.shot:focus-visible {
  transform: translateY(-2px);
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  box-shadow: 0 0.5rem 1.5rem color-mix(in srgb, var(--accent) 14%, transparent);
}

.shot:disabled {
  cursor: default;
}

.shot img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
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
  padding: 1rem;
  text-align: center;
}

.key {
  position: absolute;
  top: 0.5rem;
  left: 0.5rem;
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.1rem 0.5rem;
  border-radius: 999px;
  background: rgb(8 10 20 / 0.72);
  backdrop-filter: blur(6px);
  color: #fff;
  font-size: 0.72rem;
  letter-spacing: 0.04em;
}

.picked .shot {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent);
}

.passed .shot {
  opacity: 0.55;
}

.under {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
  min-width: 0;
}

.line {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  margin: 0;
  font-size: 0.8rem;
}

.tag {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
  font-size: 0.7rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 0 0.4rem;
}

.tag i {
  font-size: 0.7em;
}

.peek {
  display: inline-flex;
  align-items: center;
  gap: 0.1rem;
  margin-left: auto;
}

.icon {
  display: inline-grid;
  place-items: center;
  width: 1.75rem;
  height: 1.75rem;
  border: 0;
  border-radius: 0.4rem;
  background: none;
  color: var(--text-muted);
  font: inherit;
  cursor: pointer;
  text-decoration: none;
}

.icon:hover,
.icon:focus-visible {
  color: var(--accent);
  background: color-mix(in srgb, var(--accent) 10%, transparent);
}

.credit {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
  margin: 0;
  font-size: 0.72rem;
  line-height: 1.35;
  text-wrap: pretty;
}

.credit span + span::before {
  content: ' · ';
}

.full-wrap {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  align-items: flex-start;
}

.full-wrap .credit {
  display: flex;
  flex-direction: column;
  gap: 0.05rem;
  font-size: 0.8rem;
  -webkit-line-clamp: none;
}

.full-wrap .credit span + span::before {
  content: none;
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
  gap: 0.2rem;
  font-size: 0.85rem;
  text-decoration: none;
}

.open-entry:hover {
  color: var(--accent);
}
</style>
