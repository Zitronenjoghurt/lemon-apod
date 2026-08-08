<script lang="ts" setup>
import { computed, onBeforeUnmount, onMounted, ref, useTemplateRef } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import MediaFrame from './MediaFrame.vue'
import RetryNotice from './RetryNotice.vue'
import { api, ApiError } from '@/api/client'
import type { ApodEntry, ApodSummary } from '@/api/types'
import { useFavorites } from '@/composables/useFavorites'
import { useRead } from '@/composables/useRead'
import { withInternalLinks } from '@/utils/apodLinks'
import { roleLabel } from '@/utils/credits'
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
const { isFavorite, toggle } = useFavorites()
const { isRead, dimmed, markRead, toggleRead } = useRead()

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
  <article ref="root" :class="{ faded: dimmed(date) }" class="feed-item card">
    <header class="head">
      <RouterLink :to="`/${date}`" class="plain">
        <time :datetime="date" class="muted date">
          <span v-if="!isRead(date)" aria-hidden="true" class="unread-dot" />
          {{ formatDate(date) }}
        </time>
      </RouterLink>
      <h2 class="title">{{ title || 'Untitled' }}</h2>
    </header>

    <MediaFrame v-if="media" :media="media" :title="title" max-height="min(70vh, 44rem)" />
    <Skeleton v-else height="18rem" />

    <div class="row actions">
      <Button
        :icon="isFavorite(date) ? 'pi pi-star-fill' : 'pi pi-star'"
        :label="isFavorite(date) ? 'Saved' : 'Save'"
        :severity="isFavorite(date) ? 'primary' : 'secondary'"
        outlined
        size="small"
        @click="toggle(date)"
      />
      <Button
        :icon="isRead(date) ? 'pi pi-check-circle' : 'pi pi-circle'"
        :label="isRead(date) ? 'Read' : 'Unread'"
        outlined
        severity="secondary"
        size="small"
        @click="toggleRead(date)"
      />
      <RouterLink :to="`/${date}`" class="plain">
        <Button
          icon="pi pi-arrow-up-right"
          label="Open"
          outlined
          severity="secondary"
          size="small"
          tabindex="-1"
        />
      </RouterLink>
    </div>

    <Message v-if="error && missing" :closable="false" severity="secondary">{{ error }}</Message>

    <RetryNotice v-else-if="error" :busy="loading" :message="error" severity="warn" @retry="load" />

    <div v-else-if="explanation" class="prose" @click="onInternalLink" v-html="explanation" />

    <div v-else aria-busy="true" aria-label="Loading the explanation" class="stack lines">
      <Skeleton height="0.9rem" width="100%" />
      <Skeleton height="0.9rem" width="100%" />
      <Skeleton height="0.9rem" width="60%" />
    </div>

    <dl v-if="credits.length" class="credits muted" @click="onInternalLink">
      <template v-for="(credit, index) in credits" :key="credit.label + index">
        <dt>{{ credit.label }}</dt>
        <dd v-html="credit.html" />
      </template>
    </dl>
  </article>
</template>

<style scoped>
.feed-item {
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
  padding: 1.1rem;
  content-visibility: auto;
  contain-intrinsic-size: auto 42rem;
}

.head {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.date {
  font-size: 0.82rem;
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
  margin-right: 0.4rem;
  vertical-align: 0.08em;
}

.feed-item.faded {
  opacity: 0.55;
  transition: opacity 0.25s ease;
}

.feed-item.faded:hover,
.feed-item.faded:focus-within {
  opacity: 1;
}

.actions {
  gap: 0.5rem;
  flex-wrap: wrap;
}

.plain {
  text-decoration: none;
  color: inherit;
  display: inline-flex;
}

.lines {
  gap: 0.55rem;
}

.credits {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 0.2rem 0.75rem;
  font-size: 0.85rem;
  margin: 0;
}

.credits dt {
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  padding-top: 0.15rem;
  opacity: 0.75;
}

.credits dd {
  margin: 0;
}

@media (max-width: 40rem) {
  .feed-item {
    padding: 0.9rem;
  }
}
</style>
