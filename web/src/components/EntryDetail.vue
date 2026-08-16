<script lang="ts" setup>
import { computed, nextTick, ref, watch } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import { useToast } from 'primevue/usetoast'
import MediaFrame from './MediaFrame.vue'
import EntryGrid from './EntryGrid.vue'
import { api } from '@/api/client'
import type { ApodEntry, ApodSummary, PictureAppearances } from '@/api/types'
import { useArrowKeys } from '@/composables/useArrowKeys'
import { useFavorites } from '@/composables/useFavorites'
import { useRead } from '@/composables/useRead'
import { withInternalLinks } from '@/utils/apodLinks'
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
const toast = useToast()
const { isFavorite, toggle } = useFavorites()
const { isRead, markRead, toggleRead } = useRead()
const alsoOnThisDay = ref<ApodSummary[]>([])
const encore = ref<PictureAppearances | null>(null)
const encoreGroup = ref<string>()
const encoreFailed = ref(false)
const prose = ref<HTMLElement>()

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

function saveToggle() {
  const wasSaved = isFavorite(props.entry.date)
  toggle(props.entry.date)
  toast.add({
    severity: wasSaved ? 'secondary' : 'success',
    summary: wasSaved ? 'Removed from favorites' : 'Saved to favorites',
    detail: props.entry.title,
    life: 2200,
  })
}

function readToggle() {
  const nowRead = toggleRead(props.entry.date)
  toast.add({
    severity: 'secondary',
    summary: nowRead ? 'Marked as read' : 'Marked as unread',
    detail: props.entry.title,
    life: 1800,
  })
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

async function copyLink() {
  const url = `${location.origin}/${props.entry.date}`
  try {
    await navigator.clipboard.writeText(url)
    toast.add({ severity: 'success', summary: 'Link copied', detail: url, life: 2200 })
  } catch {
    toast.add({
      severity: 'warn',
      summary: 'Could not copy',
      detail: 'Your browser blocked clipboard access.',
      life: 3000,
    })
  }
}

useArrowKeys({
  left: () => previous.value && void router.push(`/${previous.value}`),
  right: () => next.value && void router.push(`/${next.value}`),
})

watch(() => props.entry.date, loadOnThisDay, { immediate: true })

watch(() => props.entry.picture, loadEncore, { immediate: true })

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
  <article class="entry">
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

    <nav
      v-if="entry.picture && !encoreFailed"
      aria-label="Other days this picture ran"
      class="row encore"
    >
      <template v-if="encore && encore.items.length > 1">
        <RouterLink :to="`/pictures/${encore.picture.id}`" class="lead">
          <i aria-hidden="true" class="pi pi-replay" />
          Shown {{ encore.picture.appearances }} times
        </RouterLink>

        <ol class="row stops">
          <li v-for="item in encore.items" :key="item.date">
            <span v-if="item.date === entry.date" aria-current="page" class="stop here">
              {{ year(item.date) }}
            </span>
            <RouterLink v-else :title="formatDate(item.date)" :to="`/${item.date}`" class="stop">
              {{ year(item.date) }}
            </RouterLink>
          </li>
        </ol>

        <RouterLink :to="`/pictures/${encore.picture.id}`" class="all">
          What changed <i aria-hidden="true" class="pi pi-arrow-right" />
        </RouterLink>
      </template>

      <span v-else class="lead waiting">
        <i aria-hidden="true" class="pi pi-replay" />
        Shown more than once
      </span>
    </nav>

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
        <MediaFrame :media="entry.media" :title="entry.title" />

        <div class="row actions">
          <Button
            :icon="isFavorite(entry.date) ? 'pi pi-star-fill' : 'pi pi-star'"
            :label="isFavorite(entry.date) ? 'Saved' : 'Save'"
            :severity="isFavorite(entry.date) ? 'primary' : 'secondary'"
            outlined
            size="small"
            @click="saveToggle"
          />
          <Button
            v-tooltip.bottom="isRead(entry.date) ? 'Mark as unread' : 'Mark as read'"
            :icon="isRead(entry.date) ? 'pi pi-check-circle' : 'pi pi-circle'"
            :label="isRead(entry.date) ? 'Read' : 'Unread'"
            outlined
            severity="secondary"
            size="small"
            @click="readToggle"
          />
          <Button
            icon="pi pi-link"
            label="Copy link"
            outlined
            severity="secondary"
            size="small"
            @click="copyLink"
          />
          <a :href="entry.source_url" class="plain" rel="noopener" target="_blank">
            <Button
              icon="pi pi-external-link"
              label="Original"
              outlined
              severity="secondary"
              size="small"
              tabindex="-1"
            />
          </a>
          <RouterLink v-slot="{ navigate }" custom to="/random">
            <Button
              v-tooltip.bottom="'Another random entry'"
              icon="pi pi-sync"
              label="Random"
              outlined
              severity="secondary"
              size="small"
              @click="navigate"
            />
          </RouterLink>
        </div>

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
      </div>

      <div class="text-column">
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

    <section v-if="alsoOnThisDay.length" class="stack">
      <h2 class="section-title">On this day in other years</h2>
      <EntryGrid :entries="alsoOnThisDay" />
    </section>
  </article>
</template>

<style scoped>
.encore {
  gap: 0.5rem 0.7rem;
  flex-wrap: wrap;
  align-items: center;
  align-self: flex-start;
  max-width: 100%;
  padding: 0.35rem 0.75rem;
  border: 1px solid var(--border);
  border-radius: 1.1rem;
  font-size: 0.82rem;
}

.encore .lead {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  color: var(--text-muted);
  text-decoration: none;
  white-space: nowrap;
}

.encore .lead:hover {
  color: var(--text);
}

.encore .waiting {
  opacity: 0.7;
  padding-block: 0.05rem;
}

.encore .stops {
  list-style: none;
  margin: 0;
  padding: 0;
  gap: 0.15rem;
  flex-wrap: wrap;
  align-items: center;
}

.encore .stop {
  display: inline-block;
  padding: 0.05rem 0.4rem;
  border-radius: 999px;
  text-decoration: none;
  font-variant-numeric: tabular-nums;
  color: var(--text-muted);
}

.encore a.stop:hover {
  color: var(--text);
  background: color-mix(in srgb, var(--text) 10%, transparent);
}

.encore .stop.here {
  color: var(--accent);
  font-weight: 600;
  background: color-mix(in srgb, var(--accent) 16%, transparent);
}

.encore .all {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
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

.entry {
  display: flex;
  flex-direction: column;
  gap: var(--gap);
}

.head {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.justify {
  justify-content: space-between;
}

.when {
  display: inline-flex;
  align-items: baseline;
  gap: 0.35rem;
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
  gap: 0.4rem;
  flex: none;
}

.layout {
  display: grid;
  gap: var(--gap);
}

@media (min-width: 62rem) {
  .layout {
    grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
    gap: 2rem;
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
  gap: 0.9rem;
  min-width: 0;
}

.text-column {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  min-width: 0;
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

  .actions {
    order: 2;
  }

  .prose {
    order: 3;
  }

  .tags {
    order: 4;
  }

  .credits {
    order: 5;
  }

  .teaser {
    order: 6;
  }
}

.actions {
  gap: 0.5rem;
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}

.credits {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 0.2rem 0.75rem;
  font-size: 0.92rem;
  margin: 0;
}

.credits dt {
  font-size: 0.74rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  padding-top: 0.15rem;
  opacity: 0.75;
}

.credits dd {
  margin: 0;
}

.rights {
  display: inline-block;
  margin-left: 0.5rem;
  padding: 0.05rem 0.45rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  font-size: 0.72rem;
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
  gap: 0.4rem;
}

.tag {
  cursor: pointer;
  transition: color 0.15s ease;
}

.tags a:hover .tag {
  color: var(--accent);
}

.teaser {
  font-size: 0.9rem;
  margin: 0;
}

.section-title {
  font-size: 1.15rem;
  font-weight: 600;
  margin-top: 1rem;
}

.hits {
  position: sticky;
  top: var(--header-h);
  z-index: 4;
  gap: 0.5rem;
  font-size: 0.85rem;
  flex-wrap: wrap;
  padding: 0.4rem 0.7rem;
  border: 1px solid var(--border);
  border-radius: 999px;
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
  font-size: 0.8rem;
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
    padding: 0.45rem 0.6rem;
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
  border-radius: 0.2rem;
  padding: 0 0.1em;
  scroll-margin-block: 5rem;
}

.entry .prose .search-hit.current {
  background: var(--accent);
  color: var(--bg);
}
</style>
