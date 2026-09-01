<script lang="ts" setup>
import { computed, nextTick, ref, watch } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import EntryActions from './EntryActions.vue'
import MediaFrame from './MediaFrame.vue'
import EntryGrid from './EntryGrid.vue'
import FieldChange from './FieldChange.vue'
import { api } from '@/api/client'
import type { ApodEntry, ApodSummary, PictureAppearances } from '@/api/types'
import { useArrowKeys } from '@/composables/useArrowKeys'
import { useRead } from '@/composables/useRead'
import { apodPageUrl, withInternalLinks } from '@/utils/apodLinks'
import { licenseName, roleLabel } from '@/utils/credits'
import {
  archivePath,
  FIRST_ENTRY,
  formatDate,
  formatMonth,
  monthDay,
  nextDay,
  previousDay,
  year,
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
const alsoOnThisDay = ref<ApodSummary[]>([])
const encore = ref<PictureAppearances | null>(null)
const encoreGroup = ref<string>()
const encoreFailed = ref(false)
const prose = ref<HTMLElement>()

const migrationOpen = ref(false)

const FIELD_NAMES: Record<string, string> = {
  title: 'the title',
  explanation_text: 'the explanation',
  credit_text: 'the credit',
  has_copyright: 'the copyright note',
  license_url: 'the licence link',
  tomorrow_teaser: "tomorrow's teaser",
  media_kind: 'the file format',
}

const changed = computed(() => props.entry.changed ?? [])

const changedNames = computed(() =>
  changed.value.map((row) => FIELD_NAMES[row.field] ?? row.field.replace(/_/g, ' ')),
)

const absent = computed(() => props.entry.absent === true)

const migration = computed(() =>
  absent.value
    ? { icon: 'pi-ban', lead: "Missing from APOD's modernized site" }
    : { icon: 'pi-arrow-right-arrow-left', lead: "Changed through APOD's modernization" },
)

const terms = computed(() => (props.highlight ? queryTerms(props.highlight) : []))

const linked = computed(() => withInternalLinks(props.entry.explanation_html))

const painted = computed(() =>
  terms.value.length ? highlightHtml(linked.value, terms.value) : { html: linked.value, count: 0 },
)

const explanation = computed(() => painted.value.html)

const title = computed(() =>
  terms.value.length ? highlightText(props.entry.title, terms.value).html : null,
)

const hits = computed(() => painted.value.count)

const credits = computed(() =>
  (props.entry.credits ?? []).map((credit) => ({
    label: roleLabel(credit.role),
    html: withInternalLinks(credit.html),
  })),
)

const license = computed(() =>
  props.entry.license_url
    ? { url: props.entry.license_url, name: licenseName(props.entry.license_url) }
    : null,
)

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

async function loadEncore() {
  const group = props.entry.picture

  if (!group) {
    encore.value = null
    encoreGroup.value = undefined
    encoreFailed.value = false
    return
  }

  if (encoreGroup.value === group) return

  encore.value = null
  encoreGroup.value = group
  encoreFailed.value = false

  try {
    const found = await api.picture(group)
    if (encoreGroup.value === group) encore.value = found
  } catch {
    if (encoreGroup.value === group) encoreFailed.value = true
  }
}

const stops = ref<HTMLElement>()

async function centreCurrentStop() {
  await nextTick()
  const rail = stops.value
  const here = rail?.querySelector<HTMLElement>('.stop.here')
  if (!rail || !here) return

  const railBox = rail.getBoundingClientRect()
  const hereBox = here.getBoundingClientRect()
  rail.scrollLeft += hereBox.left - railBox.left - (railBox.width - hereBox.width) / 2
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

watch(() => props.entry.picture, loadEncore, { immediate: true })

watch(encore, centreCurrentStop)

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
            <i aria-hidden="true" class="pi pi-calendar" />
          </RouterLink>
          <nav aria-label="Adjacent days" class="row nav">
            <RouterLink v-if="previous" v-slot="{ navigate }" :to="`/${previous}`" custom>
              <Button
                v-tooltip.bottom="'Previous day (←)'"
                aria-label="Previous day"
                icon="pi pi-chevron-left"
                outlined
                rounded
                severity="secondary"
                @click="navigate"
              />
            </RouterLink>
            <RouterLink v-if="next" v-slot="{ navigate }" :to="`/${next}`" custom>
              <Button
                v-tooltip.bottom="'Next day (→)'"
                aria-label="Next day"
                icon="pi pi-chevron-right"
                outlined
                rounded
                severity="secondary"
                @click="navigate"
              />
            </RouterLink>
          </nav>
        </div>
        <h1 v-if="title" class="title" v-html="title" />
        <h1 v-else class="title">{{ entry.title }}</h1>
      </header>

      <div v-if="(entry.picture && !encoreFailed) || changed.length || absent" class="meta">
        <nav
          v-if="entry.picture && !encoreFailed"
          aria-label="Other days this picture ran"
          class="rail encore"
        >
          <template v-if="encore && encore.items.length > 1">
            <div class="row encore-row">
              <RouterLink
                v-tooltip.bottom="`Shown ${encore.picture.appearances} times`"
                :aria-label="`Shown ${encore.picture.appearances} times`"
                :to="`/pictures/${encore.picture.id}`"
                class="lead"
              >
                <i aria-hidden="true" class="pi pi-replay" />
                {{ encore.picture.appearances }}
              </RouterLink>

              <ol ref="stops" class="stops">
                <li v-for="item in encore.items" :key="item.date">
                  <span v-if="item.date === entry.date" aria-current="page" class="stop here">
                    <span aria-hidden="true" class="dot" />
                    <span class="year">{{ year(item.date) }}</span>
                  </span>
                  <RouterLink
                    v-else
                    :title="formatDate(item.date)"
                    :to="`/${item.date}`"
                    class="stop"
                  >
                    <span aria-hidden="true" class="dot" />
                    <span class="year">{{ year(item.date) }}</span>
                  </RouterLink>
                </li>
              </ol>

              <RouterLink :to="`/pictures/${encore.picture.id}`" class="all">
                What changed <i aria-hidden="true" class="pi pi-arrow-right" />
              </RouterLink>
            </div>
          </template>

          <span v-else class="lead waiting">
            <i aria-hidden="true" class="pi pi-replay" />
            Shown more than once
          </span>
        </nav>

        <div v-if="changed.length || absent" class="rail migration">
          <button
            :aria-expanded="migrationOpen"
            class="row migrated"
            type="button"
            @click="migrationOpen = !migrationOpen"
          >
            <span class="lead">
              <i :class="['pi', migration.icon]" aria-hidden="true" />
              {{ migration.lead }}
            </span>
            <span v-if="changed.length" class="row fields">
              <span v-for="name in changedNames" :key="name" class="what">{{ name }}</span>
            </span>
            <i aria-hidden="true" class="pi pi-chevron-down turn" />
          </button>

          <Transition name="unfold">
            <div v-if="migrationOpen" class="unfold-shell">
              <div class="differences">
                <p v-if="absent" class="gone">
                  NASA's modernized site has no page for this date. What you are reading was
                  archived from the legacy page.
                </p>
                <FieldChange v-for="row in changed" :key="row.field" :row="row" />
                <RouterLink class="about" to="/modernization">
                  More information about the modernization of the official APOD website
                  <i aria-hidden="true" class="pi pi-arrow-right" />
                </RouterLink>
              </div>
            </div>
          </Transition>
        </div>
      </div>

      <div v-if="highlight" class="row hits">
        <i aria-hidden="true" class="pi pi-search" />
        <span class="term">{{ highlight }}</span>
        <span aria-live="polite" class="muted count">
          {{ hits }} {{ hits === 1 ? 'match' : 'matches' }} in the explanation
        </span>
        <span v-if="hits" class="row step">
          <Button
            aria-label="Previous match"
            icon="pi pi-chevron-up"
            rounded
            severity="secondary"
            size="small"
            text
            @click="jump(-1)"
          />
          <Button
            aria-label="Next match"
            icon="pi pi-chevron-down"
            rounded
            severity="secondary"
            size="small"
            text
            @click="jump(1)"
          />
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

      <div class="layout">
        <div class="media-column">
          <MediaFrame :media="entry.media" :source="apodPageUrl(entry)" :title="entry.title">
            <template #credit>
              <dl v-if="credits.length" class="credits muted" @click="onInternalLink">
                <template v-for="(credit, index) in credits" :key="credit.label + index">
                  <dt>{{ credit.label }}</dt>
                  <dd>
                    <span v-html="credit.html" />
                    <span
                      v-if="index === 0 && entry.has_copyright"
                      class="rights"
                      title="Credited to a named copyright holder rather than released as public domain by NASA"
                    >
                      Copyrighted
                    </span>
                    <a
                      v-if="index === 0 && license"
                      :href="license.url"
                      class="rights"
                      rel="noopener license"
                      target="_blank"
                      title="Released under this licence rather than as public domain by NASA"
                    >
                      {{ license.name }}
                    </a>
                  </dd>
                </template>
              </dl>
            </template>

            <template #actions>
              <EntryActions :date="entry.date" :source-url="entry.source_url" :title="entry.title">
                <RouterLink
                  aria-label="Random: another entry from the archive"
                  class="act"
                  to="/random"
                >
                  <i aria-hidden="true" class="pi pi-sync" />
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

.encore-row {
  gap: var(--space-3);
  align-items: center;
  flex-wrap: nowrap;
}

.encore .lead {
  display: inline-flex;
  align-items: center;
  flex: none;
  font-variant-numeric: tabular-nums;
  gap: var(--space-1);
  color: var(--text-muted);
  text-decoration: none;
  white-space: nowrap;
  transition: color var(--dur-fast) var(--ease-out);
}

.encore .lead:hover {
  color: var(--text);
}

.encore .waiting {
  opacity: 0.7;
}

.stops {
  --dot: 0.5rem;
  display: flex;
  flex: 1 1 auto;
  min-width: 0;
  list-style: none;
  margin: 0;
  padding: var(--space-0) 0;
  overflow-x: auto;
  overscroll-behavior-x: contain;
  scrollbar-width: none;
}

.stops::-webkit-scrollbar {
  display: none;
}

.stops li {
  position: relative;
  flex: 1 0 2.6rem;
  display: flex;
  justify-content: center;
}

.stops li:not(:last-child)::before {
  content: '';
  position: absolute;
  top: calc(var(--dot) / 2);
  left: 50%;
  right: -50%;
  height: 1px;
  background: var(--border);
}

.stops .stop {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 0 var(--space-1);
  text-decoration: none;
  font-variant-numeric: tabular-nums;
  font-size: var(--text-2xs);
  line-height: 1.35;
  color: var(--text-muted);
  transition: color var(--dur-fast) var(--ease-out);
}

.stops .dot {
  width: var(--dot);
  height: var(--dot);
  border-radius: 50%;
  background: var(--bg-elevated);
  box-shadow: 0 0 0 1px var(--border) inset;
  transition:
    background var(--dur-fast) var(--ease-out),
    box-shadow var(--dur-fast) var(--ease-out),
    transform var(--dur-fast) var(--ease-out);
}

.stops a.stop:hover {
  color: var(--text);
}

.stops a.stop:hover .dot {
  background: color-mix(in srgb, var(--accent) 40%, transparent);
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 60%, var(--border)) inset;
  transform: scale(1.25);
}

.stops .stop.here {
  color: var(--accent);
  font-weight: 600;
}

.stops .stop.here .dot {
  background: var(--accent);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 26%, transparent);
}

.encore .all {
  display: inline-flex;
  align-items: center;
  flex: none;
  gap: var(--space-1);
  white-space: nowrap;
  text-decoration: none;
  color: var(--text-muted);
}

.encore .all:hover {
  color: var(--accent);
}

.encore .all i {
  font-size: 0.7em;
}

.meta {
  --rail-min: 3.15rem;

  display: flex;
  flex-direction: column;
  align-self: stretch;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: color-mix(in srgb, var(--text) 2%, transparent);
}

.migrated {
  width: 100%;
  gap: var(--space-2) var(--space-3);
  justify-content: space-between;
  padding: 0;
  border: 0;
  background: none;
  color: inherit;
  font: inherit;
  font-size: inherit;
  text-align: left;
  cursor: pointer;
  transition: color var(--dur-fast) var(--ease-out);
}

.migrated:hover,
.migrated:focus-visible {
  color: var(--text);
}

.migrated .turn {
  margin-left: auto;
}

.migrated .turn {
  font-size: 0.8em;
  transition: transform var(--dur-base) var(--ease-out);
}

.migrated[aria-expanded='true'] .turn {
  transform: rotate(180deg);
}

.migrated .lead {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  white-space: nowrap;
}

.migrated .fields {
  gap: var(--space-0);
  flex-wrap: wrap;
  align-items: center;
}

.migrated .what {
  padding: var(--space-0) var(--space-2);
  border-radius: var(--radius-pill);
  background: color-mix(in srgb, var(--text) 8%, transparent);
}

.differences {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin-top: var(--space-3);
  padding-top: var(--space-3);
  border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
}

.unfold-enter-active,
.unfold-leave-active {
  overflow: hidden;
  transition:
    height var(--dur-base) var(--ease-out),
    opacity var(--dur-base) var(--ease-out);
}

.unfold-enter-from,
.unfold-leave-to {
  height: 0;
  opacity: 0;
}

.unfold-enter-to,
.unfold-leave-from {
  height: auto;
}

.gone {
  margin: 0;
  font-size: var(--text-sm);
}

.about {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  width: fit-content;
  font-size: var(--text-xs);
  text-decoration: none;
}

.about i {
  font-size: 0.8em;
}

.entry-page {
  display: flex;
  flex-direction: column;
  gap: var(--space-7);
}

.entry {
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

.when i {
  font-size: 0.8em;
  opacity: 0;
  transition: opacity 0.15s ease;
}

.when:hover,
.when:focus-visible {
  color: var(--accent);
}

.when:hover i,
.when:focus-visible i {
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

.credits {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: var(--space-1) var(--space-3);
  font-size: var(--text-sm);
  margin: 0;
}

.credits dt {
  font-size: var(--text-xs);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  padding-top: var(--space-0);
  opacity: 0.75;
}

.credits dd {
  margin: 0;
}

.rights {
  display: inline-block;
  margin-left: var(--space-2);
  padding: var(--space-0) var(--space-2);
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  font-size: var(--text-xs);
  white-space: nowrap;
  vertical-align: 0.05em;
  text-decoration: none;
}

a.rights:hover {
  border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
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
  top: var(--header-h);
  z-index: 4;
  gap: var(--space-2);
  font-size: var(--text-sm);
  flex-wrap: wrap;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
  align-self: flex-start;
  max-width: 100%;
  background: color-mix(in srgb, var(--bg) 88%, transparent);
  backdrop-filter: blur(10px);
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
  .hits {
    align-self: stretch;
    border-radius: var(--radius);
    padding: var(--space-2) var(--space-2);
  }

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
