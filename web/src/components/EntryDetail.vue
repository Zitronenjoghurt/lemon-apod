<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import { useToast } from 'primevue/usetoast'
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
const toast = useToast()
const { isFavorite, toggle } = useFavorites()
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
  <article class="entry">
    <header class="head">
      <div class="row justify">
        <time :datetime="entry.date" class="muted">{{ formatDate(entry.date) }}</time>
        <nav class="row nav" aria-label="Adjacent days">
          <RouterLink v-if="previous" v-slot="{ navigate }" :to="`/${previous}`" custom>
            <Button
              v-tooltip.bottom="'Previous day (←)'"
              icon="pi pi-chevron-left"
              severity="secondary"
              outlined
              rounded
              aria-label="Previous day"
              @click="navigate"
            />
          </RouterLink>
          <RouterLink v-if="next" v-slot="{ navigate }" :to="`/${next}`" custom>
            <Button
              v-tooltip.bottom="'Next day (→)'"
              icon="pi pi-chevron-right"
              severity="secondary"
              outlined
              rounded
              aria-label="Next day"
              @click="navigate"
            />
          </RouterLink>
        </nav>
      </div>
      <h1 class="title">{{ entry.title }}</h1>
    </header>

    <div class="layout">
      <div class="media-column">
        <MediaFrame :media="entry.media" :title="entry.title" />

        <div class="row actions">
          <Button
            :label="isFavorite(entry.date) ? 'Saved' : 'Save'"
            :icon="isFavorite(entry.date) ? 'pi pi-star-fill' : 'pi pi-star'"
            :severity="isFavorite(entry.date) ? 'primary' : 'secondary'"
            outlined
            size="small"
            @click="saveToggle"
          />
          <Button
            label="Copy link"
            icon="pi pi-link"
            severity="secondary"
            outlined
            size="small"
            @click="copyLink"
          />
          <a class="plain" :href="entry.source_url" target="_blank" rel="noopener">
            <Button
              label="Original"
              icon="pi pi-external-link"
              severity="secondary"
              outlined
              size="small"
              tabindex="-1"
            />
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
      </div>

      <div class="text-column">
        <div class="prose" v-html="explanation" @click="onInternalLink" />

        <ul v-if="entry.keywords?.length" class="row tags">
          <li v-for="keyword in entry.keywords" :key="keyword">
            <RouterLink :to="{ name: 'search', query: { q: keyword } }" class="plain">
              <Tag :value="keyword" severity="secondary" rounded class="tag" />
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
  align-items: start;
}

@media (min-width: 62rem) {
  .layout {
    grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
    gap: 2rem;
  }

  .media-column {
    position: sticky;
    top: 4.75rem;
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
</style>
