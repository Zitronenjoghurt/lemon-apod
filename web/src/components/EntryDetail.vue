<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import MediaFrame from './MediaFrame.vue'
import EntryGrid from './EntryGrid.vue'
import { api } from '@/api/client'
import type { ApodEntry, ApodSummary } from '@/api/types'
import { useFavorites } from '@/composables/useFavorites'
import { withInternalLinks } from '@/utils/apodLinks'
import { licenseName, roleLabel } from '@/utils/credits'
import { FIRST_ENTRY, formatDate, monthDay, nextDay, previousDay } from '@/utils/date'

const props = defineProps<{ entry: ApodEntry; latest?: string }>()

const router = useRouter()
const { isFavorite, toggle } = useFavorites()
const copied = ref(false)
const alsoOnThisDay = ref<ApodSummary[]>([])

const explanation = computed(() => withInternalLinks(props.entry.explanation_html))

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

async function copyLink() {
  try {
    await navigator.clipboard.writeText(`${location.origin}/${props.entry.date}`)
    copied.value = true
    setTimeout(() => (copied.value = false), 1600)
  } catch {}
}

function onKey(event: KeyboardEvent) {
  if (event.metaKey || event.ctrlKey || event.altKey) return
  const target = event.target as HTMLElement | null
  if (target && ['INPUT', 'TEXTAREA'].includes(target.tagName)) return

  if (event.key === 'ArrowLeft' && previous.value) router.push(`/${previous.value}`)
  if (event.key === 'ArrowRight' && next.value) router.push(`/${next.value}`)
}

watch(() => props.entry.date, loadOnThisDay, { immediate: true })
onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => window.removeEventListener('keydown', onKey))
</script>

<template>
  <article class="stack">
    <header class="head">
      <div class="row justify">
        <time :datetime="entry.date" class="muted">{{ formatDate(entry.date) }}</time>
        <nav class="row nav">
          <RouterLink
            v-if="previous"
            :to="`/${previous}`"
            class="icon-link"
            title="Previous day (←)"
            aria-label="Previous day"
          >
            <i class="pi pi-chevron-left" aria-hidden="true" />
          </RouterLink>
          <RouterLink
            v-if="next"
            :to="`/${next}`"
            class="icon-link"
            title="Next day (→)"
            aria-label="Next day"
          >
            <i class="pi pi-chevron-right" aria-hidden="true" />
          </RouterLink>
        </nav>
      </div>
      <h1 class="title">{{ entry.title }}</h1>
    </header>

    <MediaFrame :media="entry.media" :title="entry.title" />

    <div class="row actions">
      <button
        type="button"
        class="action"
        :class="{ active: isFavorite(entry.date) }"
        @click="toggle(entry.date)"
      >
        <i :class="isFavorite(entry.date) ? 'pi pi-star-fill' : 'pi pi-star'" aria-hidden="true" />
        {{ isFavorite(entry.date) ? 'Saved' : 'Save' }}
      </button>
      <button type="button" class="action" @click="copyLink">
        <i :class="copied ? 'pi pi-check' : 'pi pi-link'" aria-hidden="true" />
        {{ copied ? 'Copied' : 'Copy link' }}
      </button>
      <a class="action" :href="entry.source_url" target="_blank" rel="noopener">
        <i class="pi pi-external-link" aria-hidden="true" /> Original
      </a>
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
            class="rights"
            :href="license.url"
            target="_blank"
            rel="noopener license"
            title="Released under this licence rather than as public domain by NASA"
          >
            {{ license.name }}
          </a>
        </dd>
      </template>
    </dl>

    <div class="prose" v-html="explanation" @click="onInternalLink" />

    <ul v-if="entry.keywords?.length" class="row tags">
      <li v-for="keyword in entry.keywords" :key="keyword">
        <RouterLink :to="{ name: 'search', query: { q: keyword } }" class="tag">
          {{ keyword }}
        </RouterLink>
      </li>
    </ul>

    <p v-if="entry.tomorrow_teaser" class="muted teaser">
      Tomorrow's picture: <em>{{ entry.tomorrow_teaser }}</em>
    </p>

    <section v-if="alsoOnThisDay.length" class="stack">
      <h2 class="section-title">On this day in other years</h2>
      <EntryGrid :entries="alsoOnThisDay" />
    </section>
  </article>
</template>

<style scoped>
.head {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.justify {
  justify-content: space-between;
}

.title {
  font-size: clamp(1.6rem, 1.1rem + 2vw, 2.4rem);
  font-weight: 700;
}

.nav {
  gap: 0.25rem;
}

.icon-link,
.action {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.4rem 0.75rem;
  border: 1px solid var(--border);
  border-radius: 0.6rem;
  background: var(--bg-elevated);
  color: inherit;
  text-decoration: none;
  font: inherit;
  font-size: 0.9rem;
  cursor: pointer;
  transition:
    border-color 0.15s ease,
    color 0.15s ease;
}

.icon-link:hover,
.action:hover {
  border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
  color: var(--accent);
}

.action.active {
  color: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
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
  display: inline-block;
  padding: 0.15rem 0.6rem;
  border: 1px solid var(--border);
  border-radius: 999px;
  font-size: 0.82rem;
  text-decoration: none;
  color: var(--text-muted);
}

.tag:hover {
  color: var(--accent);
  border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
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
</style>
