<script lang="ts" setup>
import { computed, onBeforeUnmount, onMounted, ref, useTemplateRef } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import EntryActions from './EntryActions.vue'
import MediaFrame from './MediaFrame.vue'
import RetryNotice from './RetryNotice.vue'
import { api, ApiError } from '@/api/client'
import type { ApodEntry, ApodSummary } from '@/api/types'
import { useRead } from '@/composables/useRead'
import { apodPageUrl, withInternalLinks } from '@/utils/apodLinks'
import { licenseName, roleLabel } from '@/utils/credits'
import { formatDate } from '@/utils/date'

const props = defineProps<{
  date: string
  summary?: ApodSummary
  preloaded?: ApodEntry
}>()

const PRELOAD_MARGIN = '900px 0px'
const DWELL_MS = 1500
const DWELL_RATIO = 0.35

const router = useRouter()
const { isRead, markRead } = useRead()

const root = useTemplateRef<HTMLElement>('root')
const entry = ref<ApodEntry | undefined>(props.preloaded)
const error = ref<string>()
const loading = ref(false)

const media = computed(() => entry.value?.media ?? props.summary?.media)
const title = computed(() => entry.value?.title ?? props.summary?.title ?? '')

const explanation = computed(() =>
  entry.value ? withInternalLinks(entry.value.explanation_html) : '',
)

const credits = computed(() =>
  (entry.value?.credits ?? []).map((credit) => ({
    label: roleLabel(credit.role),
    html: withInternalLinks(credit.html),
  })),
)

const license = computed(() =>
  entry.value?.license_url
    ? { url: entry.value.license_url, name: licenseName(entry.value.license_url) }
    : null,
)

const missing = ref(false)

async function load() {
  if (entry.value || loading.value) return

  loading.value = true
  error.value = undefined
  try {
    entry.value = await api.entry(props.date)
  } catch (thrown) {
    missing.value = thrown instanceof ApiError && thrown.notFound
    error.value = missing.value
      ? 'This entry is no longer in the archive.'
      : thrown instanceof ApiError && thrown.rateLimited
        ? thrown.message
        : 'Could not load this entry.'
  } finally {
    loading.value = false
  }
}

function onInternalLink(event: MouseEvent) {
  if (event.defaultPrevented || event.button !== 0) return
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return

  const href = (event.target as HTMLElement | null)?.closest('a')?.getAttribute('href')
  if (!href?.startsWith('/')) return

  event.preventDefault()
  router.push(href)
}

let preloader: IntersectionObserver | undefined
let dwell: IntersectionObserver | undefined
let timer: ReturnType<typeof setTimeout> | undefined

onMounted(() => {
  const element = root.value
  if (!element) return

  if (!entry.value) {
    preloader = new IntersectionObserver(
      ([seen]) => {
        if (!seen?.isIntersecting) return
        preloader?.disconnect()
        void load()
      },
      { rootMargin: PRELOAD_MARGIN },
    )
    preloader.observe(element)
  }

  dwell = new IntersectionObserver(
    ([seen]) => {
      clearTimeout(timer)
      if (seen && seen.intersectionRatio >= DWELL_RATIO) {
        timer = setTimeout(() => markRead(props.date), DWELL_MS)
      }
    },
    { threshold: [0, DWELL_RATIO] },
  )
  dwell.observe(element)
})

onBeforeUnmount(() => {
  preloader?.disconnect()
  dwell?.disconnect()
  clearTimeout(timer)
})
</script>

<template>
  <article ref="root" class="feed-item card">
    <header class="head">
      <RouterLink :to="`/${date}`" class="plain">
        <time :datetime="date" class="muted date">
          <span v-if="!isRead(date)" aria-hidden="true" class="unread-dot" />
          {{ formatDate(date) }}
        </time>
      </RouterLink>
      <h2 class="title">
        <RouterLink :to="`/${date}`">{{ title || 'Untitled' }}</RouterLink>
      </h2>
    </header>

    <MediaFrame
      v-if="media"
      :entry="`/${date}`"
      :media="media"
      :source="entry ? apodPageUrl(entry) : undefined"
      :title="title"
      max-height="min(70vh, 44rem)"
    >
      <template #credit>
        <dl v-if="credits.length" class="credits muted" @click="onInternalLink">
          <template v-for="(credit, index) in credits" :key="credit.label + index">
            <dt>{{ credit.label }}</dt>
            <dd>
              <span v-html="credit.html" />
              <span
                v-if="index === 0 && entry?.has_copyright"
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
        <EntryActions :date="date" :source-url="entry?.source_url" :title="title">
          <RouterLink :to="`/${date}`" aria-label="Open this entry on its own page" class="act">
            <i aria-hidden="true" class="pi pi-arrow-up-right" />
            <span class="label">Open</span>
          </RouterLink>
        </EntryActions>
      </template>
    </MediaFrame>
    <Skeleton v-else height="18rem" />

    <Message v-if="error && missing" :closable="false" severity="secondary">{{ error }}</Message>

    <RetryNotice v-else-if="error" :busy="loading" :message="error" severity="warn" @retry="load" />

    <div v-else-if="explanation" class="prose" @click="onInternalLink" v-html="explanation" />

    <div v-else aria-busy="true" aria-label="Loading the explanation" class="stack lines">
      <Skeleton height="0.9rem" width="100%" />
      <Skeleton height="0.9rem" width="100%" />
      <Skeleton height="0.9rem" width="60%" />
    </div>
  </article>
</template>

<style scoped>
.feed-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-4);
  content-visibility: auto;
  contain-intrinsic-size: auto 42rem;
}

.head {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.date {
  font-size: var(--text-sm);
}

.title a {
  color: inherit;
  text-decoration: none;
}

.title a:hover {
  color: var(--accent);
}

.title {
  font-size: clamp(1.25rem, 1rem + 1.2vw, 1.7rem);
  font-weight: 650;
  text-wrap: balance;
}

.unread-dot {
  display: inline-block;
  width: 0.42rem;
  height: 0.42rem;
  border-radius: 50%;
  background: var(--accent);
  margin-right: var(--space-2);
  vertical-align: 0.08em;
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}

.lines {
  gap: var(--space-2);
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

@media (max-width: 40rem) {
  .feed-item {
    padding: var(--space-4);
  }
}
</style>
