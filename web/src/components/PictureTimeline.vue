<script lang="ts" setup>
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import EntryDiff from './EntryDiff.vue'
import type { Appearance, Changed } from '@/api/types'
import { formatDate } from '@/utils/date'

const props = defineProps<{
  appearances: Appearance[]
  current?: string
}>()

const LABELS: Array<[keyof Changed, string, string]> = [
  ['title', 'New title', 'pi-pencil'],
  ['explanation', 'Rewritten', 'pi-align-left'],
  ['credit', 'New credit', 'pi-user'],
  ['file', 'New image source', 'pi-image'],
]

function changes(changed: Changed): Array<[string, string]> {
  return LABELS.filter(([key]) => changed[key]).map(([, label, icon]) => [label, icon])
}

function gap(days?: number): string {
  if (!days) return ''
  const years = days / 365.25
  if (years >= 1.5) return `${Math.round(years)} years later`
  const months = Math.round(days / 30.44)
  if (months >= 2) return `${months} months later`
  return `${days} ${days === 1 ? 'day' : 'days'} later`
}

const untouched = computed(
  () =>
    props.appearances.filter((item, index) => index > 0 && !changes(item.changed).length).length,
)

const opened = ref(new Set<string>())

function toggle(date: string) {
  const next = new Set(opened.value)
  if (!next.delete(date)) next.add(date)
  opened.value = next
}
</script>

<template>
  <ol class="timeline">
    <li
      v-for="(item, index) in appearances"
      :key="item.date"
      :class="{ current: item.date === current }"
    >
      <div aria-hidden="true" class="marker" />

      <div class="stop">
        <p class="when">
          <RouterLink v-if="item.date !== current" :to="`/${item.date}`" class="date">
            {{ formatDate(item.date) }}
          </RouterLink>
          <span v-else class="date here">{{ formatDate(item.date) }}</span>
          <span v-if="index === 0" class="muted note">the first time</span>
          <span v-else-if="item.since_previous_days" class="muted note">
            {{ gap(item.since_previous_days) }}
          </span>
        </p>

        <p class="title">{{ item.title }}</p>

        <ul v-if="index > 0" class="tags">
          <li v-for="[label, icon] in changes(item.changed)" :key="label">
            <i :class="['pi', icon]" aria-hidden="true" /> {{ label }}
          </li>
          <li v-if="!changes(item.changed).length" class="same">Exactly as before</li>
        </ul>

        <template v-if="index > 0 && changes(item.changed).length">
          <button
            :aria-expanded="opened.has(item.date)"
            class="reveal"
            type="button"
            @click="toggle(item.date)"
          >
            <i
              :class="opened.has(item.date) ? 'pi-chevron-down' : 'pi-chevron-right'"
              aria-hidden="true"
              class="pi"
            />
            {{
              opened.has(item.date)
                ? 'Hide the differences'
                : 'Compare with the previous appearance'
            }}
          </button>

          <EntryDiff
            v-if="opened.has(item.date)"
            :after="item.date"
            :before="appearances[index - 1].date"
          />
        </template>
      </div>
    </li>
  </ol>

  <p v-if="untouched" class="muted footnote">
    {{ untouched === 1 ? 'One of these came back' : `${untouched} of these came back` }} untouched,
    the same picture and the same words.
  </p>
</template>

<style scoped>
.timeline {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
}

.timeline > li {
  position: relative;
  padding: 0 0 1.1rem 1.4rem;
  border-left: 2px solid var(--border);
}

.timeline > li:last-child {
  border-left-color: transparent;
  padding-bottom: 0;
}

.marker {
  position: absolute;
  left: -0.36rem;
  top: 0.32rem;
  width: 0.62rem;
  height: 0.62rem;
  border-radius: 50%;
  background: var(--border);
}

.timeline > li.current > .marker {
  background: var(--accent);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
}

.stop {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}

.when {
  margin: 0;
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 0.5rem;
  font-size: 0.85rem;
}

.date {
  text-decoration: none;
  font-variant-numeric: tabular-nums;
}

.date:hover {
  text-decoration: underline;
}

.date.here {
  font-weight: 600;
  color: var(--accent);
}

.note {
  font-size: 0.78rem;
}

.title {
  margin: 0;
  font-size: 0.98rem;
  text-wrap: pretty;
}

.tags {
  list-style: none;
  margin: 0.25rem 0 0;
  padding: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
}

.tags li {
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 0.15rem 0.7rem;
  font-size: 0.75rem;
  color: var(--text-muted);
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
}

.tags i {
  font-size: 0.9em;
  line-height: 1;
  flex: none;
  width: 1.45em;
  text-align: center;
}

.tags .same {
  border-style: dashed;
}

.reveal {
  align-self: flex-start;
  margin-top: 0.4rem;
  padding: 0;
  border: 0;
  background: none;
  color: var(--text-muted);
  font: inherit;
  font-size: 0.78rem;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
}

.reveal:hover,
.reveal:focus-visible {
  color: var(--accent);
}

.reveal i {
  font-size: 0.65em;
}

.footnote {
  font-size: 0.8rem;
  margin: 0.9rem 0 0;
}
</style>
