<script lang="ts" setup>
import { computed, nextTick, ref, watch } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import ApodBirthday from './ApodBirthday.vue'
import CreditLines from './CreditLines.vue'
import EncoreRail from './EncoreRail.vue'
import EntryActions from './EntryActions.vue'
import EntryGrid from './EntryGrid.vue'
import MediaFrame from './MediaFrame.vue'
import ModernizationRail from './ModernizationRail.vue'
import { api } from '@/api/client'
import type { ApodEntry, ApodSummary } from '@/api/types'
import { useArrowKeys } from '@/composables/useArrowKeys'
import { useIndexMarks } from '@/composables/useIndexMarks'
import { usePreferences } from '@/composables/usePreferences'
import { useRead } from '@/composables/useRead'
import { apodPageUrl, withInternalLinks } from '@/utils/apodLinks'
import { objectTargets, withIndexLinks } from '@/utils/indexLinks'
import {
  archivePath,
  FIRST_ENTRY,
  formatDate,
  formatMonth,
  monthDay,
  nextDay,
  previousDay,
} from '@/utils/date'
import { highlightHtml, highlightText, HIT_CLASS } from '@/utils/highlight'
import { queryTerms } from '@/utils/searchQuery'

const props = defineProps<{
  entry: ApodEntry
  latest?: string
  highlight?: string
}>()

const router = useRouter()
const { markRead } = useRead()
const { encoreRail, modernizationRail } = usePreferences()
const alsoOnThisDay = ref<ApodSummary[]>([])
const encoreFailed = ref(false)
const prose = ref<HTMLElement>()

const changed = computed(() => props.entry.changed ?? [])
const absent = computed(() => props.entry.absent === true)

const showsEncore = computed(
  () => encoreRail.value && Boolean(props.entry.picture) && !encoreFailed.value,
)
const showsModernization = computed(
  () => modernizationRail.value && (changed.value.length > 0 || absent.value),
)

const terms = computed(() => (props.highlight ? queryTerms(props.highlight) : []))

const { contributors, mentions } = useIndexMarks(computed(() => props.entry.date))

const linked = computed(() =>
  withIndexLinks(withInternalLinks(props.entry.explanation_html), objectTargets(mentions.value)),
)

const painted = computed(() =>
  terms.value.length ? highlightHtml(linked.value, terms.value) : { html: linked.value, count: 0 },
)

const explanation = computed(() => painted.value.html)

const title = computed(() =>
  terms.value.length ? highlightText(props.entry.title, terms.value).html : null,
)

const hits = computed(() => painted.value.count)

function onInternalLink(event: MouseEvent) {
  if (event.defaultPrevented || event.button !== 0) return
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return

  const anchor = (event.target as HTMLElement | null)?.closest('a')
  const href = anchor?.getAttribute('href')
  if (!href?.startsWith('/')) return

  event.preventDefault()
  router.push(href)
}

const previous = computed(() =>
  props.entry.date > FIRST_ENTRY ? previousDay(props.entry.date) : null,
)
const next = computed(() => {
  const candidate = nextDay(props.entry.date)
  return candidate && (!props.latest || candidate <= props.latest) ? candidate : null
})

async function loadOnThisDay() {
  try {
    const entries = await api.onThisDay(monthDay(props.entry.date))
    alsoOnThisDay.value = entries.filter((item) => item.date !== props.entry.date)
  } catch {
    alsoOnThisDay.value = []
  }
}

const at = ref(0)

function jump(step: number) {
  const marks = [...(prose.value?.querySelectorAll<HTMLElement>(`.${HIT_CLASS}`) ?? [])]
  if (!marks.length) return

  at.value = (at.value + step + marks.length) % marks.length
  const mark = marks[at.value]!

  marks.forEach((node) => node.classList.remove('current'))
  mark.classList.add('current')
  mark.scrollIntoView({ block: 'center', behavior: 'smooth' })
}

function clearHighlight() {
  router.replace({ path: `/${props.entry.date}` })
}

useArrowKeys({
  left: () => previous.value && void router.push(`/${previous.value}`),
  right: () => next.value && void router.push(`/${next.value}`),
})

watch(() => props.entry.date, loadOnThisDay, { immediate: true })

watch(
  () => props.entry.picture,
  () => (encoreFailed.value = false),
)

watch(
  () => props.entry.date,
  (date) => markRead(date),
  { immediate: true },
)

watch([() => props.entry.date, hits], async () => {
  at.value = 0
  await nextTick()
  prose.value?.querySelectorAll(`.${HIT_CLASS}.current`).forEach((node) => {
    node.classList.remove('current')
  })
})
</script>

<template>
  <div class="entry-page">
    <article class="entry card">
      <header class="head">
        <div class="row justify">
          <RouterLink
            v-tooltip.bottom="`Open ${formatMonth(entry.date)} in the archive`"
            :to="archivePath(entry.date)"
            class="muted when"
          >
            <time :datetime="entry.date">{{ formatDate(entry.date) }}</time>
            <AppIcon name="calendar" />
          </RouterLink>
          <ApodBirthday :date="entry.date" />
          <nav aria-label="Adjacent days" class="row nav">
            <RouterLink v-if="previous" v-slot="{ navigate }" :to="`/${previous}`" custom>
              <Button
                v-tooltip.bottom="'Previous day (←)'"
                aria-label="Previous day"
                outlined
                rounded
                severity="secondary"
                @click="navigate"
              >
                <template #icon><AppIcon name="chevron-left" /></template>
              </Button>
            </RouterLink>
            <RouterLink v-if="next" v-slot="{ navigate }" :to="`/${next}`" custom>
              <Button
                v-tooltip.bottom="'Next day (→)'"
                aria-label="Next day"
                outlined
                rounded
                severity="secondary"
                @click="navigate"
              >
                <template #icon><AppIcon name="chevron-right" /></template>
              </Button>
            </RouterLink>
          </nav>
        </div>
        <h1 v-if="title" class="title" v-html="title" />
        <h1 v-else class="title">{{ entry.title }}</h1>
      </header>

      <div v-if="highlight" class="row hits">
        <AppIcon name="search" />
        <span class="term">{{ highlight }}</span>
        <span aria-live="polite" class="muted count">
          {{ hits }} {{ hits === 1 ? 'match' : 'matches' }} in the explanation
        </span>
        <span v-if="hits" class="row step">
          <Button
            aria-label="Previous match"
            class="hop"
            rounded
            severity="secondary"
            size="small"
            text
            @click="jump(-1)"
          >
            <template #icon><AppIcon name="chevron-up" /></template>
          </Button>
          <Button
            aria-label="Next match"
            class="hop"
            rounded
            severity="secondary"
            size="small"
            text
            @click="jump(1)"
          >
            <template #icon><AppIcon name="chevron-down" /></template>
          </Button>
        </span>
        <Button
          class="clear"
          label="Clear"
          severity="secondary"
          size="small"
          text
          @click="clearHighlight"
        />
      </div>

      <div v-if="showsEncore || showsModernization" class="meta">
        <EncoreRail
          v-if="showsEncore && entry.picture"
          :date="entry.date"
          :picture="entry.picture"
          @failed="encoreFailed = true"
        />
        <ModernizationRail v-if="showsModernization" :absent="absent" :changed="changed" />
      </div>

      <div class="layout">
        <div class="media-column">
          <MediaFrame :media="entry.media" :source="apodPageUrl(entry)" :title="entry.title">
            <template v-if="entry.credits?.length" #credit>
              <CreditLines
                :contributors="contributors"
                :credits="entry.credits"
                :has-copyright="entry.has_copyright"
                :license-url="entry.license_url"
              />
            </template>

            <template #actions>
              <EntryActions :date="entry.date" :source-url="entry.source_url" :title="entry.title">
                <RouterLink
                  aria-label="Random: another entry from the archive"
                  class="act"
                  to="/random"
                >
                  <AppIcon name="random" />
                  <span class="label">Random</span>
                </RouterLink>
              </EntryActions>
            </template>
          </MediaFrame>
        </div>

        <div class="text-column">
          <div class="reading">
            <div ref="prose" class="prose" @click="onInternalLink" v-html="explanation" />

            <ul v-if="entry.keywords?.length" class="row tags">
              <li v-for="keyword in entry.keywords" :key="keyword">
                <RouterLink :to="{ name: 'search', query: { q: keyword } }" class="plain">
                  <Tag :value="keyword" class="tag" rounded severity="secondary" />
                </RouterLink>
              </li>
            </ul>

            <p v-if="entry.tomorrow_teaser" class="muted teaser">
              Tomorrow's picture: <em>{{ entry.tomorrow_teaser }}</em>
            </p>
          </div>
        </div>
      </div>
    </article>

    <section v-if="alsoOnThisDay.length" class="stack">
      <h2 class="section-title">On this day in other years</h2>
      <EntryGrid :entries="alsoOnThisDay" />
    </section>
  </div>
</template>

<style scoped>
.rail {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-height: var(--rail-min);
  padding: var(--space-3) var(--space-4);
  border-top: 1px solid var(--border);
  color: var(--text-muted);
  font-size: var(--text-sm);
}

.rail:first-child {
  border-top: 0;
}

.meta {
  display: flex;
  flex-direction: column;
  align-self: stretch;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--text) 2%, transparent);
}

.entry-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-7);
}

.entry {
  --rail-min: 3.15rem;

  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-4);
}

@media (max-width: 40rem) {
  .entry {
    padding: var(--space-4) var(--space-3) var(--space-5);
  }
}

.head {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.justify {
  justify-content: space-between;
}

.when {
  display: inline-flex;
  align-items: baseline;
  gap: var(--space-1);
  text-decoration: none;
  border-radius: 0.4rem;
}

.when .icon {
  font-size: 0.8em;
  opacity: 0;
  transition: opacity 0.15s ease;
}

.when:hover,
.when:focus-visible {
  color: var(--accent);
}

.when:hover .icon,
.when:focus-visible .icon {
  opacity: 0.75;
}

.title {
  font-size: clamp(1.6rem, 1.1rem + 2vw, 2.4rem);
  font-weight: 700;
  text-wrap: balance;
}

.nav {
  gap: var(--space-2);
  flex: none;
}

.layout {
  display: grid;
  gap: var(--gap);
}

@media (min-width: 62rem) {
  .layout {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: var(--space-5);
    align-items: start;
  }

  .media-column {
    position: sticky;
    top: calc(var(--header-h) + 1rem);
  }
}

.media-column {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.text-column {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}

.reading {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--text) 2%, transparent);
}

.reading .tags,
.reading .teaser {
  padding-top: var(--space-3);
  border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
}

@media (max-width: 40rem) {
  .reading {
    padding: var(--space-4);
  }
}

@media (max-width: 61.99rem) {
  .layout {
    display: flex;
    flex-direction: column;
  }

  .media-column,
  .text-column {
    display: contents;
  }

  .media-column > :first-child {
    order: 1;
  }

  .reading {
    order: 3;
  }
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}

.tags {
  list-style: none;
  padding: 0;
  margin: 0;
  gap: var(--space-1);
}

.tag {
  cursor: pointer;
  padding-block: 0;
  font-size: var(--text-xs);
  transition: color var(--dur-fast) var(--ease-out);
}

.tags a:hover .tag {
  color: var(--accent);
}

.teaser {
  font-size: var(--text-sm);
  margin: 0;
}

.section-title {
  font-size: var(--text-lg);
  font-weight: 600;
  margin-top: var(--space-1);
}

.hits {
  position: sticky;
  top: calc(var(--header-h) + var(--space-2));
  z-index: 4;
  flex-direction: row;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
  min-height: var(--rail-min);
  padding: var(--space-3) var(--space-4);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--bg-elevated) 92%, var(--text));
  backdrop-filter: blur(8px);
  font-size: var(--text-sm);
}

.hits .hop,
.hits .clear {
  height: 1.65rem;
}

.hits .hop {
  width: 1.65rem;
}

.hits .term {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 16rem;
}

.hits .count {
  font-size: var(--text-sm);
}

.hits .step {
  gap: 0;
}

.hits .clear {
  margin-left: auto;
}

@media (max-width: 30rem) {
  .hits .count {
    order: 3;
    width: 100%;
  }
}
</style>

<style>
.entry .prose .search-hit,
.entry .title .search-hit {
  background: color-mix(in srgb, var(--accent) 32%, transparent);
  color: inherit;
  border-radius: var(--radius-sm);
  padding: 0 0.1em;
  scroll-margin-block: 5rem;
}

.entry .prose .search-hit.current {
  background: var(--accent);
  color: var(--bg);
}
</style>
