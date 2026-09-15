<script lang="ts" setup>
import { computed, ref, useTemplateRef, watch } from 'vue'
import { RouterLink } from 'vue-router'
import type { BallotSide } from '@/api/types'

const STAMP = new Intl.DateTimeFormat(undefined, {
  day: 'numeric',
  month: 'short',
  year: 'numeric',
})

function stamp(date: string): string {
  const at = new Date(`${date}T00:00:00Z`)
  return Number.isNaN(at.getTime()) ? date : STAMP.format(at)
}

const props = withDefaults(
  defineProps<{
    side: BallotSide
    state?: 'plain' | 'picked' | 'passed'
    disabled?: boolean
    busy?: boolean
  }>(),
  { state: 'plain', disabled: false, busy: false },
)

const emit = defineEmits<{ pick: []; zoom: [] }>()

const loaded = ref(false)
const failed = ref(false)
const picture = useTemplateRef<HTMLImageElement>('picture')

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

defineExpose({ picture })
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
        <AppIcon name="image-lost" />
        This picture could not be loaded.
      </span>
      <img
        v-show="loaded"
        ref="picture"
        :alt="side.title"
        :src="thumb"
        decoding="async"
        draggable="false"
        @error="failed = true"
        @load="loaded = true"
      />
    </button>

    <div v-if="loaded" class="under">
      <p v-if="side.credit?.length" class="credit">
        <span v-for="line in side.credit" :key="line">{{ line }}</span>
      </p>

      <p class="line">
        <time :datetime="side.date">{{ stamp(side.date) }}</time>
        <span v-if="reruns > 1" class="tag">
          <AppIcon name="replay" />
          {{ reruns }}&times;
        </span>
        <span class="spacer" />
        <button
          v-if="full"
          v-tooltip.top="'See it full size'"
          class="icon"
          type="button"
          @click="emit('zoom')"
        >
          <AppIcon v-if="busy" name="spinner" spin />
          <AppIcon v-else name="zoom" />
          <span class="sr-only">
            {{ busy ? 'Loading the full picture' : `See ${side.title} full size` }}
          </span>
        </button>
        <RouterLink v-tooltip.top="'Read this entry'" :to="`/${side.date}`" class="icon">
          <AppIcon name="book" />
          <span class="sr-only">Read the entry for {{ side.title }}</span>
        </RouterLink>
      </p>
    </div>
  </div>
</template>

<style scoped>
.rating-picture {
  position: relative;
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

.shot:hover:not(:disabled) {
  transform: translateY(-2px);
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
  box-shadow: 0 0.5rem 1.5rem color-mix(in srgb, var(--accent) 14%, transparent);
}

.shot:focus-visible {
  outline: none;
  transform: translateY(-2px);
  border-color: var(--accent);
  box-shadow:
    0 0 0 3px color-mix(in srgb, var(--accent) 45%, transparent),
    0 0.5rem 1.5rem color-mix(in srgb, var(--accent) 14%, transparent);
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
  gap: var(--space-2);
  justify-items: center;
  font-size: var(--text-sm);
  padding: var(--space-4);
  text-align: center;
}

.picked .shot {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px var(--accent);
}

.passed .shot {
  opacity: 0.55;
}

.under {
  position: absolute;
  inset: auto 0 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-0);
  padding: var(--space-5) var(--space-3) var(--space-2);
  border-radius: 0 0 var(--radius) var(--radius);
  background: linear-gradient(to top, rgb(8 10 20 / 0.82), rgb(8 10 20 / 0));
  color: #fff;
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
  pointer-events: none;
}

.under :is(button, a) {
  pointer-events: auto;
}

.line {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  flex-wrap: nowrap;
}

.line time {
  white-space: nowrap;
}

.spacer {
  margin-left: auto;
}

.credit {
  margin: 0;
  overflow: hidden;
  font-size: var(--text-2xs);
  line-height: 1.3;
  white-space: nowrap;
  text-overflow: ellipsis;
  opacity: 0.8;
}

.credit span + span::before {
  content: ' · ';
}

.tag {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--text-2xs);
  border: 1px solid rgb(255 255 255 / 0.35);
  border-radius: var(--radius-pill);
  padding: 0 var(--space-1);
}

.tag .icon {
  font-size: 0.7em;
}

.icon {
  display: inline-grid;
  place-items: center;
  width: 1.75rem;
  height: 1.75rem;
  border: 0;
  border-radius: 0.4rem;
  background: none;
  color: rgb(255 255 255 / 0.85);
  font: inherit;
  cursor: pointer;
  text-decoration: none;
}

.icon:hover,
.icon:focus-visible {
  color: #fff;
  background: rgb(255 255 255 / 0.18);
}
</style>
