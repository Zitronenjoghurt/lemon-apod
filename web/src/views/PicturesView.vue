<script lang="ts" setup>
import { onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import RetryNotice from '@/components/RetryNotice.vue'
import { api } from '@/api/client'
import type { PictureSort, SortOrder } from '@/api/types'
import { useAsync } from '@/composables/useAsync'
import { useNarrow } from '@/composables/useNarrow'
import { formatDate, year as yearOf } from '@/utils/date'

const PAGE_SIZE = 24
const DEBOUNCE_MS = 250

const SORTS: { label: string; value: `${PictureSort}:${SortOrder}` }[] = [
  { label: 'Shown most often', value: 'appearances:desc' },
  { label: 'Shown least often', value: 'appearances:asc' },
  { label: 'Longest wait between', value: 'span:desc' },
  { label: 'Came back most recently', value: 'last:desc' },
  { label: 'Oldest first appearance', value: 'first:asc' },
  { label: 'Title, A to Z', value: 'title:asc' },
]

const route = useRoute()
const router = useRouter()
const { pageLinks } = useNarrow()

const query = ref(String(route.query.q ?? ''))
const retitled = ref(route.query.retitled === '1')
const sort = ref<`${PictureSort}:${SortOrder}`>('appearances:desc')
const page = ref(Number.parseInt(String(route.query.page ?? '1'), 10) || 1)

const {
  data: listing,
  error,
  loading,
  run,
} = useAsync((signal) => {
  const [by, order] = sort.value.split(':') as [PictureSort, SortOrder]
  return api.pictures(
    {
      q: query.value || undefined,
      retitled: retitled.value || undefined,
      sort: by,
      order,
      offset: (page.value - 1) * PAGE_SIZE,
      limit: PAGE_SIZE,
    },
    signal,
  )
})

let debounce: ReturnType<typeof setTimeout> | undefined

function search(resetPage: boolean) {
  if (resetPage) page.value = 1

  clearTimeout(debounce)
  debounce = setTimeout(() => {
    void router.replace({
      name: 'pictures',
      query: {
        q: query.value || undefined,
        retitled: retitled.value ? '1' : undefined,
        page: page.value === 1 ? undefined : String(page.value),
      },
    })
    void run()
  }, DEBOUNCE_MS)
}

function onPage(event: { page: number }) {
  page.value = event.page + 1
  search(false)
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

onMounted(() => {
  void run()
})

watch(retitled, () => search(true))

function span(first: string, last: string): string {
  const from = yearOf(first)
  const to = yearOf(last)
  return from === to ? String(from) : `${from} to ${to}`
}
</script>

<template>
  <div class="stack">
    <header class="stack head">
      <h1>Encores</h1>
      <p class="muted lede">
        Some pictures are too good to be shown just once. Sometimes there is new context or a
        clearer version of the same image. This is an overview of all APODs that have been reused
        over time.
      </p>
    </header>

    <div class="row controls">
      <IconField class="search">
        <InputIcon class="pi pi-search" />
        <InputText
          v-model="query"
          aria-label="Search encores"
          fluid
          placeholder="Search titles…"
          type="search"
          @input="search(true)"
        />
      </IconField>

      <Select
        v-model="sort"
        :options="SORTS"
        aria-label="Order"
        class="sort"
        option-label="label"
        option-value="value"
        @update:model-value="search(true)"
      />

      <div class="row toggle">
        <ToggleSwitch v-model="retitled" input-id="retitled" />
        <label for="retitled">Renamed only</label>
      </div>
    </div>

    <RetryNotice v-if="error" :busy="loading" :message="error" @retry="run" />

    <h2 v-if="listing" aria-live="polite" class="count">
      {{ listing.total.toLocaleString() }} {{ listing.total === 1 ? 'encore' : 'encores' }}
      <template v-if="retitled">that were renamed</template>
    </h2>

    <div v-if="loading && !listing" class="grid">
      <Skeleton v-for="index in 8" :key="index" height="13rem" width="100%" />
    </div>

    <p v-else-if="listing && !listing.items.length" class="muted empty">No encore matches that.</p>

    <ul v-else-if="listing" class="grid">
      <li v-for="picture in listing.items" :key="picture.id">
        <RouterLink :to="`/pictures/${picture.id}`" class="card item">
          <div class="thumb">
            <img
              v-if="picture.media.thumb_url"
              :alt="picture.title"
              :src="picture.media.thumb_url"
              decoding="async"
              height="300"
              loading="lazy"
              width="480"
            />
            <div v-else class="fallback"><i aria-hidden="true" class="pi pi-image" /></div>
            <span class="tally">{{ picture.appearances }}&times;</span>
          </div>

          <div class="body">
            <p class="muted years">{{ span(picture.first, picture.last) }}</p>
            <h2 class="title">{{ picture.title }}</h2>
            <p class="muted detail">
              <span :title="`First shown ${formatDate(picture.first)}`">
                Shown {{ picture.appearances }} times
              </span>
              <span v-if="picture.titles > 1"> &middot; {{ picture.titles }} titles </span>
            </p>
          </div>
        </RouterLink>
      </li>
    </ul>

    <Paginator
      v-if="listing && listing.total > PAGE_SIZE"
      :first="(page - 1) * PAGE_SIZE"
      :page-link-size="pageLinks"
      :rows="PAGE_SIZE"
      :total-records="listing.total"
      @page="onPage"
    />
  </div>
</template>

<style scoped>
h1 {
  font-size: 1.6rem;
}

.lede {
  margin: 0;
  font-size: 0.9rem;
}

.controls {
  gap: 0.7rem;
  align-items: center;
  flex-wrap: wrap;
}

.search {
  flex: 1 1 16rem;
}

/* The field and the select are separate PrimeVue components with their own paddings, so their
   heights only agree if they are told to. */
.controls :deep(.p-inputtext),
.controls :deep(.p-select) {
  height: 2.75rem;
}

.controls :deep(.p-select-label) {
  display: flex;
  align-items: center;
}

.toggle {
  gap: 0.5rem;
  align-items: center;
  font-size: 0.88rem;
  white-space: nowrap;
}

.sort {
  flex: 0 0 auto;
}

.count {
  font-size: 0.78rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  font-weight: 600;
  margin: 0 0 -0.5rem;
}

.empty {
  padding: 2.5rem 0;
  text-align: center;
}

.grid {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 15rem), 1fr));
  gap: 1rem;
}

.item {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  text-decoration: none;
  color: inherit;
  height: 100%;
  transition:
    transform 0.18s ease,
    border-color 0.18s ease,
    box-shadow 0.18s ease;
}

.item:hover {
  transform: translateY(-2px);
  border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
  box-shadow: 0 8px 28px rgb(0 0 0 / 0.18);
}

.thumb {
  position: relative;
  aspect-ratio: 16 / 10;
  background: color-mix(in srgb, var(--text) 6%, transparent);
}

.thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.fallback {
  width: 100%;
  height: 100%;
  display: grid;
  place-items: center;
  color: var(--text-muted);
  font-size: 1.6rem;
}

.tally {
  position: absolute;
  right: 0.6rem;
  bottom: 0.6rem;
  border-radius: 999px;
  background: rgb(0 0 0 / 0.65);
  color: #fff;
  padding: 0.1rem 0.55rem;
  font-size: 0.78rem;
  font-variant-numeric: tabular-nums;
}

.body {
  padding: 0.8rem 1rem 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.years,
.detail {
  margin: 0;
  font-size: 0.78rem;
  font-variant-numeric: tabular-nums;
}

.title {
  font-size: 1rem;
  font-weight: 600;
  text-wrap: pretty;
}
</style>
